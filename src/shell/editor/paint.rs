//! src/shell/editor/paint.rs — finishing the egui frame (tessellate, upload textures,
//! paint over the swapchain) and the dev-only console REPL drain.

use super::EditorFrontend;
use crate::app::GameWorld;
use crate::editor::EditorUi;
use crate::shell::Shell;

/// How egui paints, in the window and in the headless capture alike: no MSAA, and
/// no dithering, which egui-wgpu turned on by default after 0.27. The editor kept its
/// 0.27 look through the #333 upgrade, and an undithered capture stays byte-stable.
pub(super) const EGUI_RENDERER: egui_wgpu::RendererOptions = egui_wgpu::RendererOptions {
    msaa_samples: 1,
    depth_stencil_format: None,
    dithering: false,
    predictable_texture_filtering: false,
};

impl EditorFrontend {
    /// End the egui frame and paint the whole dashboard as a load-pass over `target`.
    pub(super) fn paint(&mut self, shell: &Shell, target: &wgpu::TextureView) {
        let full_output = self.egui_ctx.end_pass();
        let paint_jobs = self
            .egui_ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [shell.renderer.config.width, shell.renderer.config.height],
            pixels_per_point: shell.window.scale_factor() as f32,
        };
        let (device, queue) = (&shell.renderer.device, &shell.renderer.queue);
        for (id, deltas) in &full_output.textures_delta.set {
            for delta in deltas {
                self.egui_renderer.update_texture(device, queue, *id, delta);
            }
        }

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Egui Render Encoder"),
        });
        self.egui_renderer
            .update_buffers(device, queue, &mut encoder, &paint_jobs, &screen);
        {
            let mut render_pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    multiview_mask: None,
                    label: Some("Egui Render Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        depth_slice: None,
                        view: target,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
                .forget_lifetime();
            self.egui_renderer
                .render(&mut render_pass, &paint_jobs, &screen);
        }
        queue.submit(std::iter::once(encoder.finish()));

        for id in &full_output.textures_delta.free {
            self.egui_renderer.free_texture(id);
        }
    }
}

/// Run a submitted REPL line through the single live evaluator. Dev builds only — the
/// console input line and the harness share `evaluate_line`.
#[cfg(feature = "dev")]
pub(super) fn drain_repl(editor_ui: &mut EditorUi, game: &GameWorld) {
    if let Some(line) = editor_ui.pending_repl.take() {
        // Hand `evaluate_line` the shared console cell (not a held borrow) so a typed
        // `print` / `Debug.*` can borrow it mid-eval without panicking (#208).
        let _ = crate::dev::console::evaluate_line(
            game.script_manager(),
            &game.resources.console,
            &line,
        );
    }
}

/// No REPL in ship builds.
#[cfg(not(feature = "dev"))]
pub(super) fn drain_repl(_editor_ui: &mut EditorUi, _game: &GameWorld) {}
