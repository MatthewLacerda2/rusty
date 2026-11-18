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
//!
//! **Stages.** The frame's CPU time is cut into named laps (`Renderer::lap`):
//! each lap is the time since the one before, so the stages sum to the frame's
//! `cpu_ms` and nothing is timed twice; the bench reports them as
//! `render.<stage>` ([`Renderer::frame_stages`]). Off unless asked for
//! ([`Renderer::time_stages`]: the dev layer's captures, which the harness and the
//! benches draw through), so the window, the editor and the player pay one branch a
//! lap. On, a lap is one clock read and a short list scan, a few dozen per frame
//! (about 1.5 µs a frame for one camera, measured on an x86_64 container).

use std::cell::{Cell, RefCell};
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

impl GpuTimer {
    /// [`Renderer::lap`] for a pass that holds the timer, not the renderer (the
    /// shadow sweeps' caster gather).
    pub(crate) fn lap(&self, stage: &'static str) {
        self.cpu.lap(stage);
    }
}

/// The CPU side of [`GpuTimer`]'s frame bracket.
#[derive(Default)]
pub(super) struct CpuFrame {
    started: Cell<Option<Instant>>,
    /// When the last lap ended (the frame's start before the first).
    lapped: Cell<Option<Instant>>,
    /// This frame's laps so far, ms by stage, in the order first lapped.
    laps: RefCell<Vec<(&'static str, f64)>>,
    cost: FrameCost,
    /// The last frame's laps.
    stages: Vec<(&'static str, f64)>,
    /// Inside [`Renderer::aside`]: nothing is timed or counted.
    pub(super) aside: bool,
    /// Whether frames are cut into laps at all ([`Renderer::time_stages`]).
    staged: bool,
}

impl CpuFrame {
    /// Start a frame; `false` (and nothing started) inside [`Renderer::aside`].
    pub(super) fn begin(&self) -> bool {
        if !self.aside {
            let now = Instant::now();
            self.started.set(Some(now));
            self.lapped.set(self.staged.then_some(now));
            self.laps.borrow_mut().clear();
        }
        !self.aside
    }

    /// Add the time since the last lap to `stage`; nothing outside a frame or
    /// inside [`Renderer::aside`].
    pub(super) fn lap(&self, stage: &'static str) {
        let Some(since) = self.lapped.get().filter(|_| !self.aside) else {
            return;
        };
        let now = Instant::now();
        let ms = (now - since).as_secs_f64() * 1000.0;
        self.lapped.set(Some(now));
        let mut laps = self.laps.borrow_mut();
        match laps.iter_mut().find(|(s, _)| *s == stage) {
            Some((_, sum)) => *sum += ms,
            None => laps.push((stage, ms)),
        }
    }

    pub(super) fn end(&mut self) {
        self.lap("finish");
        self.lapped.set(None);
        if let Some(start) = self.started.take() {
            self.cost.cpu_ms = start.elapsed().as_secs_f64() * 1000.0;
            self.cost.frames = self.cost.frames.wrapping_add(1);
            self.stages = std::mem::take(&mut *self.laps.borrow_mut());
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

    /// The last frame's CPU ms by stage, in the order the frame reached them:
    /// they sum to its [`FrameCost::cpu_ms`] but for the clock read that ends it.
    /// Empty unless [`Renderer::time_stages`] is on.
    pub fn frame_stages(&self) -> &[(&'static str, f64)] {
        &self.gpu_timer.cpu.stages
    }

    /// Cut every frame from the next into stages ([`Renderer::frame_stages`]), or stop.
    pub fn time_stages(&mut self, on: bool) {
        self.gpu_timer.cpu.staged = on;
    }

    /// End the frame's current stage as `stage` (see the module docs).
    pub(crate) fn lap(&self, stage: &'static str) {
        self.gpu_timer.lap(stage);
    }

    /// Run `draw` (a render beside the measured frame) without it counting as one:
    /// [`Renderer::frame_counters`] and [`Renderer::frame_cost`] read afterwards as
    /// they did before, and its passes take no GPU timestamps. Inside an open frame
    /// window, the GPU calls the frame had counted so far (`gpu::ops`) are kept
    /// whole, and the aside render's own are counted into nothing.
    pub fn aside<R>(&mut self, draw: impl FnOnce(&mut Self) -> R) -> R {
        let counters = self.frame_counters;
        let was = std::mem::replace(&mut self.gpu_timer.cpu.aside, true);
        let out = crate::gpu::ops::kept(|| draw(self));
        self.gpu_timer.cpu.aside = was;
        self.frame_counters = counters;
        out
    }
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod frame_tests;
