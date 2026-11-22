use std::cell::{Cell, RefCell};
use std::time::Instant;

use super::{GpuTimer, GpuTimes};
use crate::Renderer;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameCost {
    /// CPU ms from the frame's start to its end: recording it, not the GPU's work.
    pub cpu_ms: f64,
    pub frames: u64,
}

impl GpuTimer {
    /// [`Renderer::lap`] for a pass that holds the timer, not the renderer (the
    /// shadow sweeps' caster gather).
    pub(crate) fn lap(&self, stage: &'static str) {
        self.cpu.lap(stage);
    }
}

#[derive(Default)]
pub(super) struct CpuFrame {
    started: Cell<Option<Instant>>,
    lapped: Cell<Option<Instant>>,
    /// This frame's laps so far, ms by stage, in the order first lapped.
    laps: RefCell<Vec<(&'static str, f64)>>,
    cost: FrameCost,
    stages: Vec<(&'static str, f64)>,
    pub(super) aside: bool,
    staged: bool,
}

impl CpuFrame {
    pub(super) fn begin(&self) -> bool {
        if !self.aside {
            let now = Instant::now();
            self.started.set(Some(now));
            self.lapped.set(self.staged.then_some(now));
            self.laps.borrow_mut().clear();
        }
        !self.aside
    }

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
    pub(crate) fn cost(&self) -> FrameCost {
        self.cpu.cost
    }
}

impl Renderer {
    pub fn take_gpu_times(&mut self) -> Option<GpuTimes> {
        self.gpu_timer.take(&self.device)
    }

    /// Block until the GPU is done: offline callers only (captures, benches); the live loop never
    /// waits.
    pub fn wait_idle(&self) {
        let _ = self.device.poll(wgpu::PollType::wait_indefinitely());
    }

    /// Whether this renderer's device takes GPU timestamps: `false` means
    /// [`Renderer::take_gpu_times`] never returns any, not that they are late.
    pub fn times_gpu(&self) -> bool {
        self.gpu_timer.queries.is_some()
    }

    pub fn frame_cost(&self) -> FrameCost {
        self.gpu_timer.cost()
    }

    pub fn frame_stages(&self) -> &[(&'static str, f64)] {
        &self.gpu_timer.cpu.stages
    }

    pub fn time_stages(&mut self, on: bool) {
        self.gpu_timer.cpu.staged = on;
    }

    pub(crate) fn lap(&self, stage: &'static str) {
        self.gpu_timer.lap(stage);
    }

    pub(crate) fn is_aside(&self) -> bool {
        self.gpu_timer.cpu.aside
    }

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
