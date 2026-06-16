use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::{
    event::{ElementState, Event, KeyEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

use rusty::app::{GameWorld, PlayTransition};
use rusty::core::input::InputState;
use rusty::core::keymap::{Keymap, KEYBINDINGS_KEY, KEYBINDINGS_NAMESPACE};
use rusty::editor::EditorUi;
use rusty::navigation::NavigationGraph;
use rusty::render::Renderer;
use rusty::scene::Scene;
use rusty::scripting::ConsoleLogs;

/// The stable physical-key name (uppercase) the engine speaks for a winit
/// `KeyCode`, or `None` for keys the sim has no logical name for. This is the
/// hardware-side identity name; the [`Keymap`] then remaps it to a logical key
/// before the simulation ever sees it.
fn physical_key_name(key: KeyCode) -> Option<&'static str> {
    Some(match key {
        KeyCode::KeyW => "W",
        KeyCode::KeyA => "A",
        KeyCode::KeyS => "S",
        KeyCode::KeyD => "D",
        KeyCode::ArrowUp => "UP",
        KeyCode::ArrowDown => "DOWN",
        KeyCode::ArrowLeft => "LEFT",
        KeyCode::ArrowRight => "RIGHT",
        // The shoot button is just the SPACE key; the player controller script
        // edge-detects it. No engine-side "shoot" field — gameplay reads the key
        // like any other.
        KeyCode::Space => "SPACE",
        _ => return None,
    })
}

fn main() {
    env_logger::init();
    println!("[Engine] Starting rusty 3D engine...");

    // 1. Setup local asset structure inside git-ignored project folder
    std::fs::create_dir_all("project/assets/textures").ok();
    std::fs::create_dir_all("project/assets/models").ok();
    std::fs::create_dir_all("project/assets/audio").ok();
    std::fs::create_dir_all("project/scenes").ok();

    // Seed the bundled default scripts (player_controller.lua, bot.lua) into the
    // gitignored project workspace, the same way the default scene is seeded. These
    // are GAME scripts that ship with the engine; the play loop runs no gameplay.
    rusty::scene::seed_default_scripts();

    // 2. Initialize Window Event Loop
    let event_loop = EventLoop::new().unwrap();
    let window = Arc::new(
        WindowBuilder::new()
            .with_title("Rusty 3D Game Engine & Editor")
            .with_inner_size(winit::dpi::PhysicalSize::new(1280, 720))
            .build(&event_loop)
            .unwrap(),
    );

    // 3. Initialize Core Render Engine & egui Context
    let mut renderer = match pollster::block_on(Renderer::new(Arc::clone(&window))) {
        Ok(r) => r,
        Err(err) => {
            eprintln!("[Engine] Could not initialize the renderer: {err}");
            eprintln!("[Engine] No compatible GPU is available — exiting.");
            std::process::exit(1);
        }
    };
    let egui_ctx = egui::Context::default();

    // Egui state integration
    let mut egui_winit = egui_winit::State::new(
        egui_ctx.clone(),
        egui::ViewportId::ROOT,
        &window,
        Some(window.scale_factor() as f32),
        None,
    );
    let mut egui_renderer =
        egui_wgpu::Renderer::new(&renderer.device, renderer.config.format, None, 1);

    // 4. Initialize Core Engine Systems (shared simulation state)
    let scene = Rc::new(RefCell::new(Scene::new()));
    let input = Rc::new(RefCell::new(InputState::new()));
    let nav = Rc::new(RefCell::new(NavigationGraph::new(
        -20.0, 20.0, -20.0, 20.0, 1.0,
    )));
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));

    // 5. Seed and load the default scene.
    //
    // The procedural bot-chase demo used to be hand-built here (~130 lines). It now
    // lives in the tracked `assets/scenes/default.scene`, seeded into the gitignored
    // `project/scenes/` on boot (same pattern as `bot.lua`) and loaded as the boot
    // scene. The harness can boot any scene file the same way.
    let boot_scene_path = rusty::scene::seed_default_scene();
    {
        let mut s = scene.borrow_mut();
        console
            .borrow_mut()
            .info(format!("Loading scene from {}...", boot_scene_path));
        if let Err(err) = s.load_from_file(&boot_scene_path) {
            console
                .borrow_mut()
                .error(format!("Failed to load default scene: {}", err));
        }
        nav.borrow_mut().bake(&s);
    }

    // 6. Wrap the shared sim state in the window/GPU-agnostic GameWorld.
    let mut game = GameWorld::new(scene, input, nav, console);

    // Bind the persistent store to its file and load it once — this boundary read
    // is a sim input. Writes flush at Stop and on loop exit (below). The harness
    // leaves the store pathless so headless runs never read this file.
    if let Err(err) = game
        .resources
        .storage
        .borrow_mut()
        .open(rusty::core::storage::DEFAULT_STORAGE_PATH)
    {
        game.console()
            .borrow_mut()
            .error(format!("Failed to load storage: {}", err));
    }

    // Physical→logical key remap (issue #88): rebindable controls applied here, at
    // the input source, BEFORE the sim reads any key — so the simulation only ever
    // sees logical keys and stays a pure function of its inputs. Loaded from the
    // persistent store; a missing/empty binding blob means the identity (default)
    // mapping. The harness and bot-players inject logical keys directly and bypass
    // this entirely. Rebind at runtime by writing the `keybindings.bindings` blob
    // via the `Storage` API, then reloading the keymap from it.
    let keymap = match game
        .resources
        .storage
        .borrow()
        .get(KEYBINDINGS_NAMESPACE, KEYBINDINGS_KEY)
    {
        Some(blob) => Keymap::from_json(&blob),
        None => Keymap::new(),
    };

    // 7. Editor + timing state (front-end only)
    let mut editor_ui = EditorUi::new();
    // Adopt the boot scene as the current scene so Save writes back to it.
    editor_ui.current_scene_path = Some(boot_scene_path);
    let mut last_frame_time = Instant::now();
    let mut frame_count = 0;
    let mut fps = 60.0;
    let mut last_fps_update = Instant::now();

    // 8. Execute Window Event Loop
    let _ = event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Poll);

        match event {
            Event::WindowEvent {
                window_id,
                ref event,
            } if window_id == window.id() => {
                // Always feed events to egui so the header panel with Play/Stop buttons works
                let _ = egui_winit.on_window_event(&window, event);
                match event {
                    WindowEvent::CloseRequested => elwt.exit(),
                    WindowEvent::Resized(physical_size) => {
                        renderer.resize(*physical_size);
                    }
                    WindowEvent::ScaleFactorChanged { .. } => {
                        // A DPI / display change (e.g. dragging between Retina and
                        // non-Retina monitors) can invalidate the swapchain without a
                        // Resized event. Reconfigure to the window's current inner size
                        // so the surface stays valid.
                        renderer.resize(window.inner_size());
                    }
                    WindowEvent::KeyboardInput {
                        event:
                            KeyEvent {
                                physical_key: PhysicalKey::Code(key),
                                state,
                                ..
                            },
                        ..
                    } => {
                        let pressed = *state == ElementState::Pressed;
                        // Translate the physical key to its hardware name, remap it
                        // to a logical key via the keymap, then write logical state.
                        if let Some(physical) = physical_key_name(*key) {
                            let logical = keymap.resolve(physical);
                            game.input().borrow_mut().set_key_state(&logical, pressed);
                        }
                        // Hit ESC in PlayMode to unlock cursor (platform-side transition)
                        if *key == KeyCode::Escape && pressed && game.is_playing() {
                            game.set_playing(false);
                            game.console()
                                .borrow_mut()
                                .info("Unlocked cursor, entering EditorMode".to_string());
                        }
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        game.input().borrow_mut().mouse_position = (position.x, position.y);
                    }
                    WindowEvent::RedrawRequested => {
                        // Compute timing metrics
                        let now = Instant::now();
                        let delta_time = now.duration_since(last_frame_time).as_secs_f32();
                        last_frame_time = now;
                        let current_frame_duration = delta_time * 1000.0;

                        frame_count += 1;
                        if now.duration_since(last_fps_update) >= Duration::from_secs(1) {
                            fps = frame_count as f32;
                            frame_count = 0;
                            last_fps_update = now;
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
                            let raw_input = egui_winit.take_egui_input(&window);
                            egui_ctx.begin_frame(raw_input);

                            // Draw Editor dashboard UI
                            {
                                let mut s = game.world.scene.borrow_mut();
                                let mut c = game.resources.console.borrow_mut();
                                let mut n = game.resources.nav.borrow_mut();
                                editor_ui.draw(
                                    &egui_ctx,
                                    &mut s,
                                    &mut c,
                                    &mut n,
                                    &mut game.resources.is_playing,
                                    fps,
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
                                let _ = rusty::dev::console::evaluate_line(
                                    game.script_manager(),
                                    &mut c,
                                    &line,
                                );
                            }

                            let full_output = egui_ctx.end_frame();
                            let paint_jobs = egui_ctx
                                .tessellate(full_output.shapes, full_output.pixels_per_point);

                            let screen_descriptor = egui_wgpu::ScreenDescriptor {
                                size_in_pixels: [renderer.config.width, renderer.config.height],
                                pixels_per_point: window.scale_factor() as f32,
                            };

                            for (id, image_delta) in &full_output.textures_delta.set {
                                egui_renderer.update_texture(
                                    &renderer.device,
                                    &renderer.queue,
                                    *id,
                                    image_delta,
                                );
                            }

                            let mut encoder = renderer.device.create_command_encoder(
                                &wgpu::CommandEncoderDescriptor {
                                    label: Some("Egui Render Encoder"),
                                },
                            );

                            egui_renderer.update_buffers(
                                &renderer.device,
                                &renderer.queue,
                                &mut encoder,
                                &paint_jobs,
                                &screen_descriptor,
                            );

                            {
                                let mut render_pass =
                                    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                        label: Some("Egui Render Pass"),
                                        color_attachments: &[Some(
                                            wgpu::RenderPassColorAttachment {
                                                view: &view,
                                                resolve_target: None,
                                                ops: wgpu::Operations {
                                                    load: wgpu::LoadOp::Load,
                                                    store: wgpu::StoreOp::Store,
                                                },
                                            },
                                        )],
                                        depth_stencil_attachment: None,
                                        timestamp_writes: None,
                                        occlusion_query_set: None,
                                    });

                                egui_renderer.render(
                                    &mut render_pass,
                                    &paint_jobs,
                                    &screen_descriptor,
                                );
                            }

                            renderer.queue.submit(std::iter::once(encoder.finish()));

                            for id in &full_output.textures_delta.free {
                                egui_renderer.free_texture(id);
                            }
                        }

                        frame.present();
                    }
                    _ => {}
                }
            }
            Event::AboutToWait => {
                window.request_redraw();
            }
            // Quit boundary: persist the store however the loop is exiting (window
            // close, menu quit, surface OOM). A no-op when the store is pathless.
            Event::LoopExiting => {
                game.resources.flush_storage();
            }
            _ => {}
        }
    });
}
