//! src/shell/editor/start/picker.rs — the project picker as a shell prelude (#854).
//!
//! egui on the window before any project is open: [`PickerStage`] feeds it window
//! events, draws [`ProjectPicker`] each frame and opens the folder it chooses. A
//! folder that fails to open stays in the picker with the error shown.

use std::sync::Arc;

use winit::event::WindowEvent;
use winit::window::Window;

use crate::core::project::{Access, Opened};
use crate::editor::project_picker::ProjectPicker;
use crate::render::Renderer;
use crate::shell::editor::paint::{paint_pass, EGUI_RENDERER};
use crate::shell::Prelude;

/// The picker's egui and its state.
pub struct PickerStage {
    ctx: egui::Context,
    winit: egui_winit::State,
    renderer: egui_wgpu::Renderer,
    picker: ProjectPicker,
}

impl PickerStage {
    /// Set egui up on `window` around `picker`.
    pub fn new(window: &Arc<Window>, renderer: &Renderer, picker: ProjectPicker) -> Self {
        let ctx = egui::Context::default();
        let winit = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let renderer =
            egui_wgpu::Renderer::new(&renderer.device, renderer.config.format, EGUI_RENDERER);
        Self {
            ctx,
            winit,
            renderer,
            picker,
        }
    }
}

impl Prelude for PickerStage {
    type Output = Opened;

    fn on_window_event(&mut self, window: &Window, event: &WindowEvent) {
        let _ = self.winit.on_window_event(window, event);
    }

    /// Draw the picker; open what it chose. `Some` once a project is open.
    fn draw(
        &mut self,
        window: &Window,
        renderer: &Renderer,
        target: &wgpu::TextureView,
    ) -> Option<Opened> {
        let input = self.winit.take_egui_input(window);
        self.ctx.begin_pass(input);
        let chosen = self.picker.draw(&self.ctx, super::unix_now());
        let ppp = window.scale_factor() as f32;
        paint_pass(&self.ctx, &mut self.renderer, renderer, ppp, target);
        match crate::core::project::open(&chosen?, Access::Edit) {
            Ok(opened) => Some(opened),
            Err(err) => {
                self.picker
                    .fail(format!("Could not open the project: {err}"));
                None
            }
        }
    }
}

/// The picker, offscreen, to a PNG (`editor-capture --picker`): `picker` as it would
/// draw at `now`. Same contract as [`capture`](crate::shell::editor::capture::capture).
#[cfg(feature = "dev")]
pub fn capture_picker(
    picker: &mut ProjectPicker,
    now: u64,
    path: impl AsRef<std::path::Path>,
    (width, height): (u32, u32),
) -> Result<bool, String> {
    use crate::shell::editor::capture;
    let mut host = crate::dev::capture::CaptureHost::new();
    let Some(renderer) = host.renderer(width, height) else {
        return Ok(false);
    };
    let mut egui_renderer = egui_wgpu::Renderer::new(
        &renderer.device,
        crate::render::OFFSCREEN_FORMAT,
        EGUI_RENDERER,
    );
    let ctx = egui::Context::default();
    let mut output = None;
    for frame in 0..capture::DEFAULT_FRAMES {
        ctx.begin_pass(capture::raw_input(width, height, frame));
        picker.draw(&ctx, now);
        let out = output.insert(ctx.end_pass());
        for (id, deltas) in &out.textures_delta.set {
            for delta in deltas {
                egui_renderer.update_texture(&renderer.device, &renderer.queue, *id, delta);
            }
        }
        out.textures_delta.clear();
    }
    let output = output.expect("at least one frame");
    let target = capture::paint(renderer, &mut egui_renderer, &ctx, output, width, height);
    let pixels = crate::render::readback::read_texture_rgba8(
        &renderer.device,
        &renderer.queue,
        &target,
        width,
        height,
    );
    crate::render::readback::write_png(path, width, height, &pixels)?;
    Ok(true)
}
