//! crates/render/src/timing/ — how long the GPU spent on each render pass (#835).
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
//! frame or two later. Each resolved [`GpuTimes`] carries the drawn frame it measures
//! (`frame`, the renderer's [`FrameCost::frames`] count at that frame's end), so a
//! caller pairs it with its frame by index, never by when it arrived.
//!
//! **Only `TIMESTAMP_QUERY`.** Pass-boundary writes are the form every adapter with
//! timestamps supports — Apple GPUs sample only at stage boundaries, so writes inside
//! encoders or passes are out. An adapter without the feature (lavapipe, some Linux
//! drivers) gets a disabled timer: [`GpuTimer::writes`] is always `None`, nothing is
//! resolved, and the stats carry no GPU field at all — absent, not zero. The
//! headless renderer asks for the feature, and so does a `dev` build's window, whose
//! frames feed `Debug.Stats()` (#2205; `setup::limits::WINDOW`).
//!
//! **Exclusive time.** A pass's timestamps bracket its vertex work to its fragment
//! work, and GPUs overlap neighbours — a tiled GPU (Apple's) starts the next pass's
//! vertices while this one's fragments still run, so raw spans each swallow the
//! pass before them and summing them counts that time twice. Each pass is
//! therefore charged only from where the work before it ended (or from its own
//! start, if later) to its own end. The passes then add up to the frame's GPU busy
//! time (`gpu_ms`), and a heavy pass shows as heavy instead of smearing into the
//! light ones queued behind it.
//!
//! **What no row holds.** A command outside every pair (a copy, a pass recorded
//! without `timestamp_writes`) is in no row, so each resolved frame also keeps the
//! span from its first timed start to its last timed end ([`GpuTimes::span_ms`]):
//! the span less the rows ([`GpuTimes::other_ms`], the bench's `gpu.other`) is that
//! untimed work plus any wait for the CPU between the frame's submits. A stage of
//! several passes takes one pair across them ([`GpuTimer::compute_span`]): its first pass writes the start, its last the end.

mod clock;
mod frame;
mod readback;
mod times;
mod writes;

use std::cell::{Cell, RefCell};

pub use clock::ClockProbe;
pub use frame::FrameCost;
use readback::Ring;
pub use times::{GpuTimes, Pair};

rusty_core::named_enum! {
    /// The render passes the timer reports, under the stable names the stats use
    /// (each name is the pass's key in `Debug.Stats().gpu_passes`; reports diff on it).
    ///
    /// Every pass of one kind adds into its entry — the six cascade sweeps are one
    /// `shadows`, each render-texture camera's scene pass is in `forward`. New
    /// passes join a kind, or add one here (and to `docs/api/Debug.md`).
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum GpuPass {
        /// The sky's reflection cube (#1553): its capture and prefilter, on the
        /// frames the sky changed.
        SkyReflection = "sky_reflection",
        /// The sun's cascades and the point/spot shadow atlas.
        Shadows = "shadows",
        /// GPU occlusion culling (#1963), each culling camera's whole `occlusion`
        /// stage: the select, phase one's depth draw, the Hi-Z pyramid, the test,
        /// the compaction and the cluster cull, one pair across them all.
        Occlusion = "occlusion",
        /// The SSAO depth prepass (#1578), which a primed forward pass starts from.
        SsaoPrepass = "ssao_prepass",
        /// The SSAO occlusion pass, at its quality's resolution.
        Ssao = "ssao",
        /// The SSAO blur at the occlusion's resolution and its upsample to full.
        SsaoBlur = "ssao_blur",
        /// Screen-space indirect lighting (#1750): its gather, blur, accumulate and
        /// upsample before the base camera's forward pass, and the source it keeps
        /// after it.
        Ssil = "ssil",
        /// Volumetric fog (#1703): its froxels' scatter and integrate, before the
        /// base camera draws.
        VolumetricFog = "volumetric_fog",
        /// The opaque scene pass: solids, skybox.
        Forward = "forward",
        /// Screen-space reflections (#1526): the copy of the opaque image and the
        /// trace over it, right after the base camera's opaque pass. The copy is a
        /// command, not a pass, so the trace's pair holds the gap before it
        /// ([`Pair::holds_gap`]).
        Ssr = "ssr",
        /// SSR's two hit pyramids (#2028), built from the trace's targets.
        SsrPyramids = "ssr_pyramids",
        /// SSR's resolve (#2028): the hits blurred and drawn over the image.
        SsrResolve = "ssr_resolve",
        /// Translucent solids.
        Transparent = "transparent",
        /// Trails and lines.
        Ribbons = "ribbons",
        /// Particle systems.
        Particles = "particles",
        /// Depth of field (#1445): its CoC, prefilter, blur and composite passes.
        DepthOfField = "depth_of_field",
        /// Temporal anti-aliasing (#1702): its resolve, its sharpen and the clear of
        /// its reactive mask.
        Taa = "taa",
        /// Per-object motion blur (#1436): its three tile passes and the gather.
        MotionBlur = "motion_blur",
        /// The screen-space lens flare (#1443): its flare pass (the composite adds
        /// it, timed under `post_fx`).
        LensFlare = "lens_flare",
        /// The post-FX chain: bloom, composite, authored effects, FXAA.
        PostFx = "post_fx",
        /// The editor's selection outline (its mask, dilation and ring, #1175) and the
        /// gizmos drawn over it, both after post-FX (#1224).
        Outline = "outline",
        /// World and screen UI, its masks and backdrop blur.
        Ui = "ui",
    }
}

/// Query pairs one frame may hold: well past what the deepest camera stack issues.
const PAIRS: u32 = 256;

/// The renderer's GPU pass timer: disabled (every call a no-op) where the device
/// lacks `TIMESTAMP_QUERY`.
pub struct GpuTimer {
    queries: Option<Queries>,
    /// The frame bracket's CPU side, timed whether or not the GPU is ([`FrameCost`]).
    cpu: frame::CpuFrame,
    /// Test-only: every pair asked for, timer enabled or not, so a test on an
    /// adapter without timestamps still counts a frame's pairs against [`PAIRS`].
    #[cfg(test)]
    pub(crate) asked: Cell<u32>,
}

/// The live half of an enabled timer.
struct Queries {
    set: wgpu::QuerySet,
    /// Nanoseconds per timestamp tick (`Queue::get_timestamp_period`).
    period_ns: f64,
    /// Whether a frame is being recorded: passes outside `render` are not timed.
    open: Cell<bool>,
    /// This frame's query pairs, in pair order.
    frame: RefCell<Vec<Pair>>,
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
            ..Self::disabled()
        }
    }

    /// A timer that times nothing: what a device without timestamps gets.
    pub fn disabled() -> Self {
        Self {
            queries: None,
            cpu: Default::default(),
            #[cfg(test)]
            asked: Cell::new(0),
        }
    }

    /// Start timing a frame's passes.
    pub(crate) fn begin_frame(&self) {
        if !self.cpu.begin() {
            return; // a render beside the measured frame (`Renderer::aside`)
        }
        if let Some(q) = &self.queries {
            q.frame.borrow_mut().clear();
            q.open.set(true);
        }
    }

    /// Close the frame: resolve its queries into a free readback buffer and start
    /// mapping it. A frame with every buffer still in flight goes untimed.
    /// The CPU span closes last, so it holds the resolve, its submit and the poll.
    pub(crate) fn end_frame(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        // The count `cpu.end` below moves to: this frame's index.
        let frame = self.cost().frames.wrapping_add(1);
        if let Some(q) = &mut self.queries {
            q.open.set(false);
            let passes = std::mem::take(q.frame.get_mut());
            if !passes.is_empty() {
                q.ring.resolve_frame(device, queue, &q.set, passes, frame);
            }
            q.collect(device);
        }
        self.cpu.end();
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
        if let Some((frame, times)) = self
            .ring
            .read_finished(|ticks, passes| GpuTimes::from_ticks(ticks, passes, period))
        {
            self.ready = Some(GpuTimes { frame, ..times });
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(test)]
#[path = "budget_gpu_tests.rs"]
mod budget_gpu_tests;
