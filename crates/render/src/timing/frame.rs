//! What each frame the renderer draws cost on the CPU, and a render kept out of it
//! (#2205).
//!
//! The windowed loop reads the frame stats off the renderer itself: no frontend
//! times its own `render` call. [`GpuTimer`]'s frame bracket is where a frame starts
//! and ends, so it also takes the CPU time between the two ([`FrameCost`]) and
//! counts the frames, which is how a caller tells a frame that drew from one that
//! didn't. A render beside the frame being measured (the Inspector's preview, a
//! cubemap capture's faces) goes through [`Renderer::aside`]: its counters, cost
//! and GPU passes are not the frame's.

use std::time::Instant;

use super::{GpuTimer, GpuTimes};
use crate::Renderer;

/// The renderer's last frame's CPU cost, and how many frames it has drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameCost {
    /// CPU ms from the frame's start to its end: recording it, not the GPU's work.
    pub cpu_ms: f64,
    /// Frames drawn since the renderer was built: a frame drew when this moved.
    pub frames: u64,
}

/// The CPU side of [`GpuTimer`]'s frame bracket.
#[derive(Default)]
pub(super) struct CpuFrame {
    started: std::cell::Cell<Option<Instant>>,
    cost: FrameCost,
    /// Inside [`Renderer::aside`]: nothing is timed or counted.
    pub(super) aside: bool,
}

impl CpuFrame {
    /// Start a frame; `false` (and nothing started) inside [`Renderer::aside`].
    pub(super) fn begin(&self) -> bool {
        if !self.aside {
            self.started.set(Some(Instant::now()));
        }
        !self.aside
    }

    pub(super) fn end(&mut self) {
        if let Some(start) = self.started.take() {
            self.cost.cpu_ms = start.elapsed().as_secs_f64() * 1000.0;
            self.cost.frames = self.cost.frames.wrapping_add(1);
        }
    }
}

impl GpuTimer {
    /// The last frame's [`FrameCost`].
    pub(crate) fn cost(&self) -> FrameCost {
        self.cpu.cost
    }
}

impl Renderer {
    /// The newest rendered frame's GPU time per pass not yet taken — normally a frame
    /// or two behind, never waited for. `None` without timestamps (#835).
    pub fn take_gpu_times(&mut self) -> Option<GpuTimes> {
        self.gpu_timer.take(&self.device)
    }

    /// Block until the GPU has finished everything submitted so far. For offline
    /// callers only (the harness, a bake): the live frame loop never waits.
    pub fn wait_idle(&self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
    }

    /// Whether this renderer's device takes GPU timestamps: `false` means
    /// [`Renderer::take_gpu_times`] never returns any, not that they are late.
    pub fn times_gpu(&self) -> bool {
        self.gpu_timer.queries.is_some()
    }

    /// What the last frame cost the CPU, and how many frames this renderer drew.
    pub fn frame_cost(&self) -> FrameCost {
        self.gpu_timer.cost()
    }

    /// Run `draw` (a render beside the measured frame) without it counting as one:
    /// [`Renderer::frame_counters`] and [`Renderer::frame_cost`] read afterwards as
    /// they did before, and its passes take no GPU timestamps.
    pub fn aside<R>(&mut self, draw: impl FnOnce(&mut Self) -> R) -> R {
        let counters = self.frame_counters;
        let was = std::mem::replace(&mut self.gpu_timer.cpu.aside, true);
        let out = draw(self);
        self.gpu_timer.cpu.aside = was;
        self.frame_counters = counters;
        out
    }
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod frame_tests;
