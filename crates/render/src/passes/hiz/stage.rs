use super::clusters::plan::Plan;
use super::clusters::ClusterDraws;
use super::cull::CameraCull;
use super::drawn::Layout;
use super::pyramid::Pyramid;
use crate::draw::camera::CameraCtx;
use crate::gpu::ops::CountedEncoder as _;
use crate::timing::GpuPart;
use crate::{RenderView, Renderer};

impl Renderer {
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
        // Before the select and the timer's pairs: phase one draws through the global group.
        self.update_global_bind_group();
        let (timer, split) = (&self.gpu_timer, self.gpu_timer.splits());
        let pair = |part| split.then(|| timer.part_compute_writes(part)).flatten();
        let open = pair(GpuPart::OcclusionSelect);
        let encoder = ctx.commands.encoder();
        let mut select = encoder.counted_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Hi-Z Select"),
            timestamp_writes: open,
        });
        dispatch(&mut select, (&self.hiz.select, &groups[0]), batches);
        drop(select);
        self.lap(lap_of(GpuPart::OcclusionSelect));
        let phase_one = split.then(|| timer.part_writes(GpuPart::OcclusionPhaseOne));
        let phase_one = ("Hi-Z Phase One", phase_one.flatten(), CLEAR);
        let solids = (opaque, Some(&first));
        self.record_depth(encoder, &view.depth_view, solids, phase_one);
        let read = cam.counter.triangles.map(|t| t.phase_one);
        self.count_phase_one(opaque, read);
        self.lap(lap_of(GpuPart::OcclusionPhaseOne));
        let at = (&view.depth_view, ctx.view_proj);
        let cull = (&mut *cam, &mut *pyramid, &plan);
        last.clusters = self.record_cull(encoder, cull, at, (n, batches, split));
        // Built only for a read that goes: one is copied while none is in flight.
        if cam.counter.idle() {
            let clustered = last.clusters.as_ref().map_or(&[][..], |c| &c.draws[..]);
            let reading = Layout::of(opaque, clustered, plan.jobs.len());
            cam.copy_counts(&self.device, encoder, reading);
        }
        self.lap(lap_of(GpuPart::OcclusionTest));
        cam.counter.request(n, &mut ctx.commands);
        let tested = plan.tested;
        cam.clusters.counter.request(tested, &mut ctx.commands);
        let counters = &mut self.frame_counters;
        counters.hiz_culled += cam.counter.culled;
        counters.hiz_late += cam.counter.late;
        counters.cluster_tested += plan.tested;
        counters.cluster_culled += cam.clusters.counter.while_active(!plan.runs.is_empty());
        ctx.solids.culled = Some(last);
        self.lap("occlusion.record");
    }

    fn hiz_instances(&self, view: &mut RenderView, ctx: &mut CameraCtx, idx: usize) -> Option<u32> {
        (ctx.solids.culled, ctx.solids.floored) = (None, false);
        let n = ctx.solids.draws.opaque.last().map(|b| b.instances.end);
        let culls = n.is_some() && self.hiz_culls(ctx.cam);
        if culls && self.hiz_draws_enough(&ctx.solids) {
            return n;
        }
        ctx.solids.floored = culls;
        if let Some(cam) = view.hiz.cameras.get_mut(idx) {
            // Its first culled frame back must not report a read left in flight before.
            cam.forget_reads();
        }
        None
    }

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
        // Timed, a part per pass: a timestamp pair cannot open and close inside a pass.
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
    fn forget_reads(&mut self) {
        self.counter.forget();
        self.clusters.counter.forget();
    }

    fn copy_counts(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        reading: Layout,
    ) {
        let args = (
            self.args[0].buffer(),
            self.args[1].buffer(),
            self.clusters.args(),
        );
        self.counter.copy(device, encoder, &reading.segments(args));
        self.reading = reading;
    }
}

/// The far plane: phase one clears the depth, and the prepass loads what it kept.
const CLEAR: wgpu::LoadOp<f32> = crate::gpu::depth::CLEAR;

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

pub(super) const HOLD_LAP: &str = "occlusion.hold";

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

pub(super) const CLUSTER_STEPS: [GpuPart; 3] = [
    GpuPart::OcclusionClustersTest,
    GpuPart::OcclusionClustersScan,
    GpuPart::OcclusionClustersCopy,
];

fn cluster_parts(clustered: bool, split: bool) -> &'static [GpuPart] {
    match (clustered, split) {
        (false, _) => &[],
        (true, false) => &[GpuPart::OcclusionClusters],
        (true, true) => &CLUSTER_STEPS,
    }
}

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

fn label_of(part: GpuPart) -> &'static str {
    match part {
        GpuPart::OcclusionClustersTest => "occlusion.clusters_test",
        GpuPart::OcclusionClustersScan => "occlusion.clusters_scan",
        GpuPart::OcclusionClustersCopy => "occlusion.clusters_copy",
        _ => lap_of(part),
    }
}

fn dispatch(
    pass: &mut wgpu::ComputePass,
    (pipeline, group): (&crate::gpu::pipelines::LazyCompute, &wgpu::BindGroup),
    count: u32,
) {
    pass.set_pipeline(pipeline.get());
    pass.set_bind_group(crate::gpu::ONLY_GROUP, group, &[]);
    // 64: `hiz.wgsl`'s workgroup size.
    pass.dispatch_workgroups(count.div_ceil(64), 1, 1);
}
