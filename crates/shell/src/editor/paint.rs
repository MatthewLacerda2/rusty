//! crates/shell/src/editor/paint.rs — finishing the egui frame (tessellate, upload textures,
//! paint over the swapchain), the frame's edits settled, and the dev-only console
//! REPL drain.

use std::time::Duration;

use super::EditorFrontend;
use crate::Shell;
use rusty_app::GameWorld;
use rusty_editor::EditorUi;
use rusty_render::Renderer;

pub(in crate::editor) const EGUI_RENDERER: egui_wgpu::RendererOptions =
    egui_wgpu::RendererOptions {
        msaa_samples: 1,
        depth_stencil_format: None,
        dithering: false,
        predictable_texture_filtering: false,
    };

impl EditorFrontend {
    /// End the egui frame and paint the whole dashboard as a load-pass over `target`.
    pub(super) fn paint(&mut self, shell: &Shell, target: &wgpu::TextureView) {
        let ppp = shell.window.scale_factor() as f32;
        let after = paint_pass(
            &self.egui_ctx,
            &mut self.egui_renderer,
            &shell.renderer,
            ppp,
            target,
        );
        self.repaint_at = std::time::Instant::now().checked_add(after);
    }
}

/// End `ctx`'s pass and paint it as a load-pass over the swapchain `target`: the
/// editor's dashboard and the project picker alike. Returns how soon egui wants
/// its next frame (a tooltip, an animation, a caret), [`Duration::MAX`] for never.
pub(in crate::editor) fn paint_pass(
    ctx: &egui::Context,
    egui_renderer: &mut egui_wgpu::Renderer,
    renderer: &Renderer,
    pixels_per_point: f32,
    target: &wgpu::TextureView,
) -> Duration {
    let mut full_output = ctx.end_pass();
    let repaint_after = full_output
        .viewport_output
        .get(&egui::ViewportId::ROOT)
        .map_or(Duration::MAX, |root| root.repaint_delay);
    let paint_jobs = ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
    let screen = egui_wgpu::ScreenDescriptor {
        size_in_pixels: [renderer.config.width, renderer.config.height],
        pixels_per_point,
    };
    let (device, queue) = (&renderer.device, &renderer.queue);
    for (id, deltas) in &full_output.textures_delta.set {
        for delta in deltas {
            egui_renderer.update_texture(device, queue, *id, delta);
        }
    }

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Egui Render Encoder"),
    });
    egui_renderer.update_buffers(device, queue, &mut encoder, &paint_jobs, &screen);
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
        egui_renderer.render(&mut render_pass, &paint_jobs, &screen);
    }
    queue.submit(std::iter::once(encoder.finish()));

    for id in &full_output.textures_delta.free {
        egui_renderer.free_texture(id);
    }
    // All applied; egui asserts no delta is dropped unhandled.
    full_output.textures_delta.clear();
    repaint_after
}

pub(super) fn take_edits(editor_ui: &mut EditorUi, game: &GameWorld) -> bool {
    let edited = std::mem::take(&mut editor_ui.is_dirty);
    if edited && game.scene().borrow().is_playing() {
        let what = "an editor edit (the Inspector, the hierarchy, a gizmo)".to_string();
        let journal = game.script_manager().journal();
        journal
            .borrow_mut()
            .push(rusty_script::scripting::HostCall::Taint(what));
    }
    edited
}

#[cfg(feature = "dev")]
pub(super) fn drain_repl(editor_ui: &mut EditorUi, game: &GameWorld) {
    if let Some(line) = editor_ui.dev.pending_repl.take() {
        let _ = rusty_dev::console::evaluate_line(
            game.script_manager(),
            &game.resources.console,
            &line,
        );
        editor_ui.is_dirty |= !game.scene().borrow().is_playing();
    }
}

/// No REPL in ship builds.
// lint: allow(gpu-default-only) ship stub: no REPL to drain; nothing is drawn
#[cfg(not(feature = "dev"))]
pub(super) fn drain_repl(_editor_ui: &mut EditorUi, _game: &GameWorld) {}

#[cfg(all(test, feature = "dev"))]
mod tests {
    use super::*;
    use rusty_dev::harness::Harness;
    use rusty_script::scripting::HostCall;

    #[test]
    fn an_inspector_edit_during_play_taints_the_demo() {
        let h = Harness::new(rusty_test_support::temp::dir(), "");
        let mut ui = EditorUi::new(Default::default());
        let journal = h.world.borrow().script_manager().journal();
        journal.borrow_mut().start();
        ui.is_dirty = true;
        assert!(take_edits(&mut ui, &h.world.borrow()) && !ui.is_dirty);
        assert!(
            journal.borrow_mut().take().is_empty(),
            "edit mode: no taint"
        );
        h.step(1);
        ui.is_dirty = true;
        assert!(take_edits(&mut ui, &h.world.borrow()));
        let acts = journal.borrow_mut().take();
        assert!(matches!(&acts[..], [HostCall::Taint(what)] if what.contains("editor edit")));
    }
}
