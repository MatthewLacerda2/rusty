//! The `occlusion` stage (#1963, the [module docs](super)): one camera's two
//! phases recorded into one encoder and one submit, and timed whole or, on a
//! timed frame, by part (#2401): the select, phase one's depth draw, the
//! pyramid, the instance test and compaction, and the cluster cull, each its own
//! GPU pair (`gpu.occlusion.<part>`) and CPU lap (`render.occlusion.<part>`); the
//! cluster cull's GPU pair is three, one per dispatch kind (#2406).

use super::clusters::plan::Plan;
use super::clusters::ClusterDraws;
use super::cull::{late_args, CameraCull, ARGS};
use super::pyramid::Pyramid;
use crate::draw::camera::CameraCtx;
use crate::gpu::ops::{CountedEncoder as _, CountedQueue as _};
use crate::timing::GpuPart;
use crate::{RenderView, Renderer};

impl Renderer {
    /// The `occlusion` stage: cull this camera's opaque instances by the GPU's
    /// depth pyramid, leaving the draws its passes make in `ctx.solids.culled`.
    pub(crate) fn cull_by_hiz(&mut self, view: &mut RenderView, ctx: &mut CameraCtx) {
        let idx = ctx.uniform.stack[0] as usize;
        let Some(n) = self.hiz_instances(view, ctx, idx) else {
            return;
        };
        self.hold_hiz(view, idx);
        let ([first, mut last], plan) =
            self.prepare_cull(view, idx, (&ctx.solids, ctx.view_proj), n);
        let (cam, pyramid) = (&mut view.hiz.cameras[idx], &mut view.hiz.pyramid);
        let pyramid = pyramid.as_mut().expect("held above");
        let groups = cam.groups.value().expect("bound by prepare_cull");
        let opaque = &ctx.solids.draws.opaque;
        let batches = opaque.len() as u32;
        // Before the timer lends out its pairs: phase one draws through this group.
        self.update_global_bind_group();
        // The stage is one encoder and one submit. A timed frame records each part
        // in a pass and pair of its own (#2401), since a pair cannot open and
        // close inside a pass; any other frame takes no pair, so it records the
        // parts after phase one in one pass (#2327).
        let (timer, split) = (&self.gpu_timer, self.gpu_timer.splits());
        let pair = |part| split.then(|| timer.part_compute_writes(part)).flatten();
        let open = pair(GpuPart::OcclusionSelect);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let mut select = encoder.counted_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Hi-Z Select"),
            timestamp_writes: open,
        });
        dispatch(&mut select, (&self.hiz.select, &groups[0]), batches);
        drop(select);
        self.lap(lap_of(GpuPart::OcclusionSelect));
        let phase_one = split.then(|| timer.part_writes(GpuPart::OcclusionPhaseOne));
        // Phase one clears the depth: the prepass then keeps it (#2336).
        let phase_one = ("Hi-Z Phase One", phase_one.flatten(), CLEAR);
        let solids = (opaque, Some(&first));
        self.record_depth(&mut encoder, &view.depth_view, solids, phase_one);
        self.lap(lap_of(GpuPart::OcclusionPhaseOne));
        let at = (&view.depth_view, ctx.view_proj);
        let cull = (&mut *cam, &mut *pyramid, &plan);
        last.clusters = self.record_cull(&mut encoder, cull, at, (n, batches, split));
        cam.copy_counts(&self.device, &mut encoder, batches);
        self.lap(lap_of(GpuPart::OcclusionTest));
        self.queue.counted_submit(std::iter::once(encoder.finish()));
        cam.counter.request(n);
        cam.clusters.counter.request(plan.tested);
        let counters = &mut self.frame_counters;
        counters.hiz_culled += cam.counter.culled;
        counters.hiz_late += cam.counter.late;
        counters.cluster_tested += plan.tested;
        counters.cluster_culled += cam.clusters.counter.while_active(!plan.runs.is_empty());
        ctx.solids.culled = Some(last);
        self.lap("occlusion.submit");
    }

    /// How many of camera `idx`'s instances its stage culls; `None` when it is
    /// drawn whole, `ctx.solids.culled` left `None`. Under the instance floor,
    /// or with its flag off, a camera holds nothing new here (#2338), and a
    /// read it left in flight is dropped: its first frame back must not report
    /// it. Under the floor it is [`floored`](crate::draw::resources::SolidResources::floored):
    /// its forward pass is still primed, by a prepass of its few batches.
    fn hiz_instances(&self, view: &mut RenderView, ctx: &mut CameraCtx, idx: usize) -> Option<u32> {
        (ctx.solids.culled, ctx.solids.floored) = (None, false);
        let n = ctx.solids.draws.opaque.last().map(|b| b.instances.end);
        let culls = n.is_some() && self.hiz_culls(ctx.cam);
        if culls && self.hiz_draws_enough(&ctx.solids) {
            return n;
        }
        ctx.solids.floored = culls;
        if let Some(cam) = view.hiz.cameras.get_mut(idx) {
            cam.forget_reads();
        }
        None
    }

    /// Record everything after phase one: the pyramid over `depth`, the test,
    /// the compaction and `plan`'s cluster cull. When `split` (the stage's one
    /// read of `GpuTimer::splits`) in one pass per part, each its own pair (#2401);
    /// otherwise in one compute pass (#2327), wgpu ordering the dispatches
    /// inside it, each reading what the one before wrote.
    /// Laps the CPU's parts. Returns the clustered batches' draws.
    fn record_cull(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        (cam, pyramid, plan): (&mut CameraCull, &mut Pyramid, &Plan),
        (depth, view_proj): (&wgpu::TextureView, glam::Mat4),
        (n, batches, split): (u32, u32, bool),
    ) -> Option<ClusterDraws> {
        let clustered = !plan.runs.is_empty();
        if clustered {
            let at = (&*pyramid, cam.visible.buffer(), view_proj);
            self.ready_clusters((&mut cam.clusters, plan), at);
            self.lap(lap_of(GpuPart::OcclusionClusters));
        }
        let groups = cam.groups.value().expect("bound by prepare_cull");
        let clusters = &cam.clusters;
        let mut record = |pass: &mut wgpu::ComputePass, part| {
            match part {
                GpuPart::OcclusionPyramid => pyramid.build(&self.device, &self.hiz, pass, depth),
                GpuPart::OcclusionTest => {
                    dispatch(pass, (&self.hiz.test, &groups[1]), n);
                    dispatch(pass, (&self.hiz.compact, &groups[1]), batches);
                }
                GpuPart::OcclusionSelect | GpuPart::OcclusionPhaseOne => {
                    unreachable!("recorded by `cull_by_hiz` before phase one ends")
                }
                // The cluster cull whole, or one of its dispatch kinds.
                cluster @ (GpuPart::OcclusionClusters
                | GpuPart::OcclusionClustersTest
                | GpuPart::OcclusionClustersScan
                | GpuPart::OcclusionClustersCopy) => {
                    self.record_clusters(pass, (clusters, plan), steps_of(cluster));
                }
            }
            self.lap(lap_of(part));
        };
        let after = [GpuPart::OcclusionPyramid, GpuPart::OcclusionTest];
        let parts = after
            .into_iter()
            .chain(cluster_parts(clustered, split).iter().copied());
        if split {
            for part in parts {
                let writes = self.gpu_timer.part_compute_writes(part);
                record(&mut begin(encoder, label_of(part), writes), part);
            }
        } else {
            let mut pass = begin(encoder, "Hi-Z Cull", None);
            parts.for_each(|part| record(&mut pass, part));
        }
        let cull = (&mut cam.clusters, plan);
        let drawn = clustered.then(|| self.clusters_drawn(encoder, cull, batches as usize));
        if clustered {
            self.lap(lap_of(GpuPart::OcclusionClusters));
        }
        drawn
    }
}

impl CameraCull {
    /// Drop both counters' reads in flight, for a frame this camera skips.
    fn forget_reads(&mut self) {
        self.counter.forget();
        self.clusters.counter.forget();
    }

    /// Copy what its final draws and its late set (#2336) drew, `batches` args
    /// each and adjacent ([`late_args`]), in one copy for one read back
    /// ([`Counter::final_and_late`](super::counters::Counter::final_and_late)).
    fn copy_counts(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        batches: u32,
    ) {
        let bytes = late_args(batches as usize) + ARGS * u64::from(batches);
        self.counter
            .copy(device, encoder, (self.args[1].buffer(), 0, bytes));
    }
}

/// Phase one's depth load: cleared to the far plane.
const CLEAR: wgpu::LoadOp<f32> = wgpu::LoadOp::Clear(1.0);

/// A compute pass of `encoder`, labelled, its pair's `writes` when timed.
fn begin(
    encoder: &mut wgpu::CommandEncoder,
    label: &'static str,
    writes: Option<wgpu::ComputePassTimestampWrites<'_>>,
) -> wgpu::ComputePass<'static> {
    let desc = wgpu::ComputePassDescriptor {
        label: Some(label),
        timestamp_writes: writes,
    };
    encoder.counted_compute_pass(&desc).forget_lifetime()
}

/// The stage's CPU lap before its parts: making room for the camera (and the
/// pyramid's allocation on a resize) and collecting last frame's counters, kept
/// apart so a resize spike is never read as a part's.
pub(super) const HOLD_LAP: &str = "occlusion.hold";

/// The CPU lap a part of the stage is timed in, and its pass's label when it
/// is a pass of its own: `render.occlusion.<part>` in the bench (#2401). The
/// CPU parts are the work for each: `test` holds packing the instances' bounds
/// and copying the verdict count, `clusters` planning, uploading and copying
/// the cluster cull's, and `select` the uploads and binds before its pass.
pub(super) fn lap_of(part: GpuPart) -> &'static str {
    match part {
        GpuPart::OcclusionSelect => "occlusion.select",
        GpuPart::OcclusionPhaseOne => "occlusion.phase_one",
        GpuPart::OcclusionPyramid => "occlusion.pyramid",
        GpuPart::OcclusionTest => "occlusion.test",
        GpuPart::OcclusionClusters
        | GpuPart::OcclusionClustersTest
        | GpuPart::OcclusionClustersScan
        | GpuPart::OcclusionClustersCopy => "occlusion.clusters",
    }
}

/// The cluster cull's three stages, in the order they run, each a sub-part of
/// `gpu.occlusion.clusters` on a timed frame (#2406).
pub(super) const CLUSTER_STEPS: [GpuPart; 3] = [
    GpuPart::OcclusionClustersTest,
    GpuPart::OcclusionClustersScan,
    GpuPart::OcclusionClustersCopy,
];

/// The cluster cull's parts: none without a clustered batch; untimed, one, its
/// three stages in turn; timed, a pass per stage. Either way one test, one scan
/// and one copy dispatch for the frame (`clusters::record`, #2417).
fn cluster_parts(clustered: bool, split: bool) -> &'static [GpuPart] {
    match (clustered, split) {
        (false, _) => &[],
        (true, false) => &[GpuPart::OcclusionClusters],
        (true, true) => &CLUSTER_STEPS,
    }
}

/// The dispatch kinds the cluster cull's `part` records: all three for the
/// part itself, else the one it names; none for a part outside the cull.
fn steps_of(part: GpuPart) -> &'static [GpuPart] {
    match part {
        GpuPart::OcclusionClusters => &CLUSTER_STEPS,
        GpuPart::OcclusionClustersTest => &CLUSTER_STEPS[0..1],
        GpuPart::OcclusionClustersScan => &CLUSTER_STEPS[1..2],
        GpuPart::OcclusionClustersCopy => &CLUSTER_STEPS[2..3],
        GpuPart::OcclusionSelect
        | GpuPart::OcclusionPhaseOne
        | GpuPart::OcclusionPyramid
        | GpuPart::OcclusionTest => &[],
    }
}

/// A timed part's pass label: its lap's, but a cluster cull dispatch's own.
fn label_of(part: GpuPart) -> &'static str {
    match part {
        GpuPart::OcclusionClustersTest => "occlusion.clusters_test",
        GpuPart::OcclusionClustersScan => "occlusion.clusters_scan",
        GpuPart::OcclusionClustersCopy => "occlusion.clusters_copy",
        _ => lap_of(part),
    }
}

/// One dispatch of `pipeline` over `group` in `pass`, a thread per one of `count`.
fn dispatch(
    pass: &mut wgpu::ComputePass,
    (pipeline, group): (&crate::gpu::pipelines::LazyCompute, &wgpu::BindGroup),
    count: u32,
) {
    pass.set_pipeline(pipeline.get());
    pass.set_bind_group(0, group, &[]);
    pass.dispatch_workgroups(count.div_ceil(64), 1, 1);
}
