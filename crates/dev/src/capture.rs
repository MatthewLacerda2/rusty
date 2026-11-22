use std::path::Path;
use std::time::Instant;

use rusty_core::settings::render_scale::RenderScale;
use rusty_render::{
    gpu_required, readback, GpuTimes, RenderCounters, RenderView, Renderer, ADAPTER_ENV,
    OFFSCREEN_FORMAT, REQUIRE_GPU_ENV,
};
use rusty_scene::{Camera, Scene};

pub fn no_adapter(what: &str) -> Result<(), String> {
    if gpu_required() {
        return Err(format!(
            "{REQUIRE_GPU_ENV}=1 but no GPU or software adapter was found (or none \
             matches {ADAPTER_ENV}, when set) — cannot {what}"
        ));
    }
    log::warn!("[Capture] no GPU/software adapter available — skipping {what}");
    Ok(())
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct RenderedFrame {
    pub counters: RenderCounters,
    pub cpu_ms: f64,
    pub stages: Vec<(&'static str, f64)>,
    pub gpu: Option<GpuTimes>,
}

/// Holds a slot of the headless budget (`render::setup::budget`) while its renderer
/// lives: own one per batch of work, never one per process.
#[derive(Default)]
pub struct CaptureHost {
    renderer: Option<Renderer>,
    view: Option<RenderView>,
    probed: bool,
    pub last_frame: Option<RenderedFrame>,
    pub exposure: Option<f32>,
    pub idle_at: Option<Instant>,
    pub gizmos: bool,
}

impl CaptureHost {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn frame(
        &mut self,
        width: u32,
        height: u32,
        render_scale: RenderScale,
    ) -> Option<(&mut Renderer, &mut RenderView)> {
        let (width, height) = (width.max(1), height.max(1));
        self.renderer(width, height)?;
        let renderer = self.renderer.as_mut()?;
        let view = RenderView::ensure_offscreen(
            &mut self.view,
            &renderer.device,
            OFFSCREEN_FORMAT,
            (width, height),
            render_scale,
        );
        Some((renderer, view))
    }

    pub fn renderer(&mut self, width: u32, height: u32) -> Option<&mut Renderer> {
        if !self.probed {
            self.probed = true;
            let (width, height) = (width.max(1), height.max(1));
            // Built at the first shot's size; later sizes work because the offscreen path reads
            // no size off the renderer's `config`.
            self.renderer = pollster::block_on(Renderer::new_headless(width, height));
            if let Some(r) = self.renderer.as_mut() {
                r.time_stages(true);
            }
        }
        self.renderer.as_mut()
    }

    pub fn draw(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        width: u32,
        height: u32,
    ) -> Result<Option<RenderedFrame>, String> {
        let scale = self.renderer(width, height).map(|r| r.render_scale(false));
        let gizmos = self.gizmos;
        let Some((renderer, view)) = self.frame(width, height, scale.unwrap_or_default()) else {
            no_adapter("draw a capture")?;
            return Ok(None);
        };
        view.game_gizmos = gizmos;
        let target = view
            .color_target_view()
            .ok_or_else(|| "capture view owns no colour target".to_string())?;
        renderer.render(view, scene, camera, &target, false);
        let cpu_ms = renderer.frame_cost().cpu_ms;
        renderer.wait_idle();
        let idle_at = crate::stats::now();
        let exposure = view.settled_exposure(&renderer.device);
        let frame = RenderedFrame {
            counters: renderer.frame_counters,
            cpu_ms,
            stages: renderer.frame_stages().to_vec(),
            gpu: renderer.take_gpu_times(),
        };
        self.last_frame = Some(frame.clone());
        self.exposure = exposure;
        self.idle_at = Some(idle_at);
        Ok(Some(frame))
    }

    pub fn write_png(&self, path: impl AsRef<Path>) -> Result<(), String> {
        let (Some(renderer), Some(view)) = (self.renderer.as_ref(), self.view.as_ref()) else {
            return Err("capture host has no rendered frame to write".to_string());
        };
        let target = view
            .color_target()
            .ok_or_else(|| "capture view owns no colour target".to_string())?;
        let size = view.size();
        let pixels = readback::read_texture_rgba8(
            &renderer.device,
            &renderer.queue,
            target,
            size.width,
            size.height,
        );
        readback::write_png(path, size.width, size.height, &pixels)
    }
}

#[cfg(test)]
#[path = "capture_tests.rs"]
mod capture_tests;

#[cfg(test)]
#[path = "capture_required_tests.rs"]
mod capture_required_tests;
