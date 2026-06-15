//! src/render_loop.rs — the windowed per-frame tick (sim advance + GPU render + egui).
//!
//! The body of the `RedrawRequested` arm of the winit event loop, pulled out of
//! `main.rs` to keep both files under the size cap. `main.rs` owns the event loop
//! closure and calls `run_frame` once per redraw; the `winit` event-loop wiring,
//! window/GPU construction, and input handling stay in `main.rs`.

use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::event_loop::EventLoopWindowTarget;
use winit::window::Window;

use rusty::app::{GameWorld, PlayTransition};
use rusty::editor::EditorUi;
use rusty::render::Renderer;

/// Mutable timing accumulators threaded across frames by `main.rs`.
pub struct FrameTiming {
    pub last_frame_time: Instant,
    pub frame_count: u32,
    pub fps: f32,
    pub last_fps_update: Instant,
}

/// Advance the simulation one frame, render the 3D scene, and draw the egui
/// overlay. Called from the `RedrawRequested` arm of the event loop. Returns
/// early (identically to the original closure `return`s) on recoverable surface
/// errors or to exit the loop.
#[allow(clippy::too_many_arguments)]
pub fn run_frame(
    elwt: &EventLoopWindowTarget<()>,
    window: &Arc<Window>,
    renderer: &mut Renderer,
    game: &mut GameWorld,
    editor_ui: &mut EditorUi,
    egui_ctx: &egui::Context,
    egui_winit: &mut egui_winit::State,
    egui_renderer: &mut egui_wgpu::Renderer,
    timing: &mut FrameTiming,
) {
    // Compute timing metrics
    let now = Instant::now();
    let delta_time = now.duration_since(timing.last_frame_time).as_secs_f32();
    timing.last_frame_time = now;
    let current_frame_duration = delta_time * 1000.0;

    timing.frame_count += 1;
    if now.duration_since(timing.last_fps_update) >= Duration::from_secs(1) {
        timing.fps = timing.frame_count as f32;
        timing.frame_count = 0;
        timing.last_fps_update = now;
    }

    // Advance the simulation (decoupled from window + GPU).
    let transition = game.tick(delta_time);
    match transition {
        PlayTransition::Entered => {
            window
                .set_cursor_grab(winit::window::CursorGrabMode::Confined)
                .ok();
            window.set_cursor_visible(false);
        }
        PlayTransition::Exited => {
            window
                .set_cursor_grab(winit::window::CursorGrabMode::None)
                .ok();
            window.set_cursor_visible(true);
        }
        PlayTransition::None => {}
    }

    // --- GPU RENDER TICK ---
    let surface_frame = {
        let window_surface = renderer
            .surface
            .as_ref()
            .expect("windowed renderer must have a surface");
        window_surface.get_current_texture()
    };
    // Variant-specific surface recovery. `Lost`/`Outdated` only
    // recover by reconfiguring the surface — which never happens on
    // the per-frame path otherwise (only on Resized), so a surface
    // lost without a resize event would log forever. Reuse
    // `resize` (it reconfigures and guards zero-size) to recover.
    let frame = match surface_frame {
        Ok(f) => f,
        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
            renderer.resize(renderer.size);
            return;
        }
        Err(wgpu::SurfaceError::OutOfMemory) => {
            eprintln!("[WGPU] Surface out of memory — exiting");
            elwt.exit();
            return;
        }
        Err(wgpu::SurfaceError::Timeout) => return,
    };
    let view = frame
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    // Render the 3D scene (Forward unlit/lit + gizmos line drawers)
    {
        let s = game.scene().borrow();
        let cam = game.camera().borrow();
        renderer.render(
            &s,
            &cam,
            &view,
            !game.is_playing(),
            game.pathfinding_points(),
        );
    }

    // Render Egui Overlay (header always visible, side panels only in editor mode)
    {
        let raw_input = egui_winit.take_egui_input(window);
        egui_ctx.begin_frame(raw_input);

        // Draw Editor dashboard UI
        {
            let mut s = game.world.scene.borrow_mut();
            let mut c = game.resources.console.borrow_mut();
            let mut n = game.resources.nav.borrow_mut();
            editor_ui.draw(
                egui_ctx,
                &mut s,
                &mut c,
                &mut n,
                &mut game.resources.is_playing,
                timing.fps,
                current_frame_duration,
            );
            if editor_ui.is_dirty {
                renderer.shadow_renderer.is_static_cached = false;
                editor_ui.is_dirty = false;
            }
            // Apply the selected post-FX scalability tier.
            renderer.set_quality(editor_ui.quality_preset);
        }

        // Drain a submitted REPL line through the single live
        // evaluator. Dev builds only — the console input line
        // and the harness share `dev::console::evaluate_line`.
        #[cfg(feature = "dev")]
        if let Some(line) = editor_ui.pending_repl.take() {
            let mut c = game.resources.console.borrow_mut();
            let _ = rusty::dev::console::evaluate_line(game.script_manager(), &mut c, &line);
        }

        let full_output = egui_ctx.end_frame();
        let paint_jobs = egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [renderer.config.width, renderer.config.height],
            pixels_per_point: window.scale_factor() as f32,
        };

        for (id, image_delta) in &full_output.textures_delta.set {
            egui_renderer.update_texture(&renderer.device, &renderer.queue, *id, image_delta);
        }

        let mut encoder =
            renderer
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Egui Render Encoder"),
                });

        egui_renderer.update_buffers(
            &renderer.device,
            &renderer.queue,
            &mut encoder,
            &paint_jobs,
            &screen_descriptor,
        );

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Egui Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            egui_renderer.render(&mut render_pass, &paint_jobs, &screen_descriptor);
        }

        renderer.queue.submit(std::iter::once(encoder.finish()));

        for id in &full_output.textures_delta.free {
            egui_renderer.free_texture(id);
        }
    }

    frame.present();
}
