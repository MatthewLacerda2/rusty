//! src/render/timing/ — how long the GPU spent on each render pass (#835).
//!
//! Every other timing in the frame stats is CPU-side: `renderer_ms` is how long
//! `Renderer::render` took to *record* the frame, not how long the GPU took to draw
//! it. This module asks the GPU. Each timed render pass gets a pair of timestamp
//! queries (its descriptor's `timestamp_writes`: start and end of the pass), tagged
//! with the [`GpuPass`] it belongs to. At the end of `Renderer::render` the frame's
//! queries are resolved and copied into one of a small ring of mappable buffers,
//! mapped asynchronously. A later call finds the map done, turns the ticks into
//! milliseconds per pass and keeps them for the dev layer to [`GpuTimer::take`].
//!
//! **Never stalls the frame.** Nothing here waits on the GPU: the device is only
//! polled, and a frame that finds every readback buffer still in flight is simply
//! not timed. A caller that has waited for the GPU anyway (the harness, which reads
//! its frames back) gets the frame's times on the next `take`; otherwise they land a
//! frame or two later.
//!
//! **Only `TIMESTAMP_QUERY`.** Pass-boundary writes are the form every adapter with
//! timestamps supports — Apple GPUs sample only at stage boundaries, so writes inside
//! encoders or passes are out. An adapter without the feature (lavapipe, some Linux
//! drivers) gets a disabled timer: [`GpuTimer::writes`] is always `None`, nothing is
//! resolved, and the stats carry no GPU field at all — absent, not zero. Only the
//! headless renderer asks for the feature: nothing reads the window's timings yet.
//!
//! **Exclusive time.** A pass's timestamps bracket its vertex work to its fragment
//! work, and GPUs overlap neighbours — a tiled GPU (Apple's) starts the next pass's
//! vertices while this one's fragments still run, so raw spans each swallow the
//! pass before them and summing them counts that time twice. Each pass is
//! therefore charged only from where the work before it ended (or from its own
//! start, if later) to its own end. The passes then add up to the frame's GPU busy
//! time (`gpu_ms`), and a heavy pass shows as heavy instead of smearing into the
//! light ones queued behind it.

mod readback;

use std::cell::{Cell, RefCell};

use readback::Ring;

/// The render passes the timer reports, under the stable names the stats use.
///
/// Every pass of one kind adds into its entry — the six cascade sweeps are one
/// `shadows`, each render-texture camera's scene pass is in `forward`. New
/// passes join a kind, or add one here (and to `docs/api/Debug.md`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GpuPass {
    /// The sun's cascades and the point/spot shadow atlas.
    Shadows,
    /// The SSAO depth prepass, occlusion and blur.
    Ssao,
    /// The opaque scene pass: solids, skybox, editor overlays.
    Forward,
    /// Translucent solids.
    Transparent,
    /// Trails and lines.
    Ribbons,
    /// Particle systems.
    Particles,
    /// The post-FX chain: bloom, composite, authored effects, FXAA.
    PostFx,
    /// World and screen UI, its masks and backdrop blur.
    Ui,
}

impl GpuPass {
    /// Every pass, in frame order.
    pub const ALL: [GpuPass; 8] = [
        GpuPass::Shadows,
        GpuPass::Ssao,
        GpuPass::Forward,
        GpuPass::Transparent,
        GpuPass::Ribbons,
        GpuPass::Particles,
        GpuPass::PostFx,
        GpuPass::Ui,
    ];

    /// The pass's key in `Debug.Stats().gpu_passes`. Stable: reports diff on it.
    pub fn name(self) -> &'static str {
        match self {
            GpuPass::Shadows => "shadows",
            GpuPass::Ssao => "ssao",
            GpuPass::Forward => "forward",
            GpuPass::Transparent => "transparent",
            GpuPass::Ribbons => "ribbons",
            GpuPass::Particles => "particles",
            GpuPass::PostFx => "post_fx",
            GpuPass::Ui => "ui",
        }
    }
}

/// One frame's GPU milliseconds per pass kind, in [`GpuPass::ALL`] order. A kind
/// with no pass that frame is left out.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GpuTimes {
    pub passes: Vec<(GpuPass, f64)>,
}

impl GpuTimes {
    /// The frame's GPU time: every timed pass's, summed.
    pub fn total_ms(&self) -> f64 {
        self.passes.iter().map(|(_, ms)| ms).sum()
    }

    /// Sum resolved `ticks` — a begin/end pair per entry of `passes`, in the order
    /// the passes were queued — into exclusive milliseconds per pass kind (see the
    /// module doc). `period_ns` is nanoseconds per tick. A pass that ends before the
    /// work queued ahead of it counts zero.
    pub fn from_ticks(ticks: &[u64], passes: &[GpuPass], period_ns: f64) -> Self {
        let mut ms = [None::<f64>; GpuPass::ALL.len()];
        let mut done = 0u64; // where the work queued so far ended
        for (pair, pass) in ticks.chunks_exact(2).zip(passes) {
            let (begin, end) = (pair[0].max(done), pair[1]);
            done = done.max(end);
            let elapsed = end.saturating_sub(begin) as f64 * period_ns / 1e6;
            let slot = &mut ms[*pass as usize];
            *slot = Some(slot.unwrap_or(0.0) + elapsed);
        }
        let passes = GpuPass::ALL
            .iter()
            .zip(ms)
            .filter_map(|(pass, ms)| Some((*pass, ms?)))
            .collect();
        Self { passes }
    }
}

/// Query pairs one frame may hold: well past what the deepest camera stack issues.
const PAIRS: u32 = 256;

/// The renderer's GPU pass timer: disabled (every call a no-op) where the device
/// lacks `TIMESTAMP_QUERY`.
pub struct GpuTimer {
    queries: Option<Queries>,
}

/// The live half of an enabled timer.
struct Queries {
    set: wgpu::QuerySet,
    /// Nanoseconds per timestamp tick (`Queue::get_timestamp_period`).
    period_ns: f64,
    /// Whether a frame is being recorded: passes outside `render` are not timed.
    open: Cell<bool>,
    /// This frame's timed passes, one per query pair, in pair order.
    frame: RefCell<Vec<GpuPass>>,
    /// Warned that a frame ran out of query pairs (once per timer).
    overflowed: Cell<bool>,
    ring: Ring,
    /// The newest resolved frame not yet taken.
    ready: Option<GpuTimes>,
}

impl GpuTimer {
    /// A timer for `device`: enabled when it was created with `TIMESTAMP_QUERY`.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return Self::disabled();
        }
        let set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("GPU Pass Timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: PAIRS * 2,
        });
        let queries = Queries {
            set,
            period_ns: f64::from(queue.get_timestamp_period()),
            open: Cell::new(false),
            frame: RefCell::new(Vec::new()),
            overflowed: Cell::new(false),
            ring: Ring::new(device, u64::from(PAIRS) * 2),
            ready: None,
        };
        Self {
            queries: Some(queries),
        }
    }

    /// A timer that times nothing: what a device without timestamps gets.
    pub fn disabled() -> Self {
        Self { queries: None }
    }

    /// Start timing a frame's passes.
    pub(crate) fn begin_frame(&self) {
        if let Some(q) = &self.queries {
            q.frame.borrow_mut().clear();
            q.open.set(true);
        }
    }

    /// The `timestamp_writes` for one render pass of kind `pass`; `None` when the
    /// timer is disabled, no frame is open, or the frame ran out of query pairs.
    pub(crate) fn writes(&self, pass: GpuPass) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        let q = self.queries.as_ref().filter(|q| q.open.get())?;
        let mut frame = q.frame.borrow_mut();
        let pair = frame.len() as u32;
        if pair >= PAIRS {
            if !q.overflowed.replace(true) {
                log::warn!("[GpuTimer] a frame ran past {PAIRS} timed passes; the rest go untimed");
            }
            return None;
        }
        frame.push(pass);
        Some(wgpu::RenderPassTimestampWrites {
            query_set: &q.set,
            beginning_of_pass_write_index: Some(pair * 2),
            end_of_pass_write_index: Some(pair * 2 + 1),
        })
    }

    /// Close the frame: resolve its queries into a free readback buffer and start
    /// mapping it. A frame with every buffer still in flight goes untimed.
    pub(crate) fn end_frame(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let Some(q) = &mut self.queries else {
            return;
        };
        q.open.set(false);
        let passes = std::mem::take(q.frame.get_mut());
        if !passes.is_empty() {
            q.ring.submit(device, queue, &q.set, passes);
        }
        q.collect(device);
    }

    /// The newest frame's GPU times not yet taken, polling (never waiting) for
    /// finished readbacks first. `None` while disabled or nothing new resolved.
    pub fn take(&mut self, device: &wgpu::Device) -> Option<GpuTimes> {
        let q = self.queries.as_mut()?;
        q.collect(device);
        q.ready.take()
    }
}

impl Queries {
    /// Poll the device and fold every finished readback into `ready`.
    fn collect(&mut self, device: &wgpu::Device) {
        let _ = device.poll(wgpu::PollType::Poll);
        let period = self.period_ns;
        if let Some(times) = self
            .ring
            .read_finished(|ticks, passes| GpuTimes::from_ticks(ticks, passes, period))
        {
            self.ready = Some(times);
        }
    }
}

impl crate::render::Renderer {
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
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
