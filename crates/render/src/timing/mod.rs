mod clock;
mod frame;
mod readback;
mod times;
mod writes;

use std::cell::{Cell, RefCell};

pub use clock::ClockProbe;
pub use frame::FrameCost;
use readback::Ring;
use times::Pair;
pub use times::{GpuPart, GpuTimes};

rusty_core::named_enum! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub enum GpuPass {
        SkyReflection = "sky_reflection",
        Shadows = "shadows",
        Occlusion = "occlusion",
        DepthPrepass = "depth_prepass",
        Ssao = "ssao",
        SsaoBlur = "ssao_blur",
        Ssil = "ssil",
        VolumetricFog = "volumetric_fog",
        Forward = "forward",
        Ssr = "ssr",
        SsrPyramids = "ssr_pyramids",
        SsrResolve = "ssr_resolve",
        Transparent = "transparent",
        Ribbons = "ribbons",
        Particles = "particles",
        DepthOfField = "depth_of_field",
        Taa = "taa",
        MotionBlur = "motion_blur",
        LensFlare = "lens_flare",
        PostFx = "post_fx",
        Outline = "outline",
        Ui = "ui",
    }
}

const PAIRS: u32 = 256;

pub struct GpuTimer {
    queries: Option<Queries>,
    /// The frame bracket's CPU side, timed whether or not the GPU is ([`FrameCost`]).
    cpu: frame::CpuFrame,
    #[cfg(test)]
    pub(crate) asked: Cell<u32>,
    #[cfg(test)]
    pub(crate) split_override: Cell<Option<bool>>,
}

struct Queries {
    set: wgpu::QuerySet,
    /// Nanoseconds per timestamp tick (`Queue::get_timestamp_period`).
    period_ns: f64,
    /// Whether a frame is being recorded: passes outside `render` are not timed.
    open: Cell<bool>,
    frame: RefCell<Vec<Pair>>,
    /// Warned that a frame ran out of query pairs (once per timer).
    overflowed: Cell<bool>,
    ring: Ring,
    /// The newest resolved frame not yet taken.
    ready: Option<GpuTimes>,
}

impl GpuTimer {
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

    pub fn disabled() -> Self {
        Self {
            queries: None,
            cpu: Default::default(),
            #[cfg(test)]
            asked: Cell::new(0),
            #[cfg(test)]
            split_override: Cell::new(None),
        }
    }

    pub(crate) fn splits(&self) -> bool {
        #[cfg(test)]
        if let Some(split) = self.split_override.get() {
            return split;
        }
        self.queries.as_ref().is_some_and(|q| q.open.get())
    }

    pub(crate) fn begin_frame(&self) {
        if !self.cpu.begin() {
            return;
        }
        if let Some(q) = &self.queries {
            q.frame.borrow_mut().clear();
            q.open.set(true);
        }
    }

    pub(crate) fn end_frame(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
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
#[path = "parts_tests.rs"]
mod parts_tests;

#[cfg(test)]
#[path = "budget_gpu_tests.rs"]
mod budget_gpu_tests;
