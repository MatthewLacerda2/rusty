//! The `occlusion` stage (#1963, the [module docs](super)): one camera's two
//! phases recorded into one encoder and one submit, and timed whole or, on a
//! timed frame, by part (#2401): the select, phase one's depth draw, the
//! pyramid, the instance test and compaction, and the cluster cull, each its own
//! GPU pair (`gpu.occlusion.<part>`) and CPU lap (`render.occlusion.<part>`).

use super::clusters::plan::Plan;
use super::clusters::ClusterDraws;
use super::cull::{CameraCull, ARGS};
use super::pyramid::Pyramid;
use crate::draw::camera::CameraCtx;
use crate::gpu::ops::{CountedEncoder as _, CountedQueue as _};
use crate::timing::{GpuPart, GpuPass};
use crate::{RenderView, Renderer};

impl Renderer {
    /// The `occlusion` stage: cull this camera's opaque instances by the GPU's
    /// depth pyramid, leaving the draws its passes make in `ctx.solids.culled`.
    pub(crate) fn cull_by_hiz(&mut self, view: &mut RenderView, ctx: &mut CameraCtx) {
        ctx.solids.culled = None;
        let opaque = ctx.solids.draws.opaque.last();
        let Some(last) = opaque.filter(|_| self.hiz_culls(ctx.cam)) else {
            return;
        };
        let n = last.instances.end;
        let idx = ctx.uniform.stack[0] as usize;
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
        // The stage is one encoder and one submit, and a pair spread over passes
        // is charged only inside one submit (`timing::writes`). Untimed, its one
        // `occlusion` pair opens on the select's pass and closes on the merged
        // pass; on a timed frame each part is its own pass and pair (#2401).
        let (timer, split) = (&self.gpu_timer, self.gpu_timer.splits());
        let (open, close) = match split {
            true => (timer.part_compute_writes(GpuPart::OcclusionSelect), None),
            false => timer.compute_span(GpuPass::Occlusion),
        };
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let mut select = encoder.counted_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Hi-Z Select"),
            timestamp_writes: open,
        });
        dispatch(&mut select, (&self.hiz.select, &groups[0]), batches);
        drop(select);
        self.lap(lap_of(GpuPart::OcclusionSelect));
        let phase_one = split.then(|| timer.part_writes(GpuPart::OcclusionPhaseOne));
        let phase_one = ("Hi-Z Phase One", phase_one.flatten());
        let solids = (opaque, Some(&first));
        self.record_depth(&mut encoder, &view.depth_view, solids, phase_one);
        self.lap(lap_of(GpuPart::OcclusionPhaseOne));
        let at = (&view.depth_view, ctx.view_proj);
        let cull = (&mut *cam, &mut *pyramid, &plan);
        last.clusters = self.record_cull(&mut encoder, cull, at, (n, batches), close);
        let args = (cam.args[1].buffer(), ARGS * u64::from(batches));
        cam.counter.copy(&self.device, &mut encoder, args);
        self.lap(lap_of(GpuPart::OcclusionTest));
        self.queue.counted_submit(std::iter::once(encoder.finish()));
        cam.counter.request(n);
        cam.clusters.counter.request(plan.tested);
        let counters = &mut self.frame_counters;
        counters.hiz_culled += cam.counter.culled;
        counters.cluster_tested += plan.tested;
        counters.cluster_culled += cam.clusters.counter.while_active(!plan.runs.is_empty());
        ctx.solids.culled = Some(last);
        self.lap("occlusion.submit");
    }

    /// Record everything after phase one: the pyramid over `depth`, the test,
    /// the compaction and `plan`'s cluster cull. Untimed, in one compute pass
    /// (#2327) whose end writes `close`, wgpu ordering the dispatches inside it,
    /// each reading what the one before wrote; on a timed frame
    /// (`GpuTimer::splits`), in one pass per part, each its own pair (#2401).
    /// Laps the CPU's parts. Returns the clustered batches' draws.
    fn record_cull(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        (cam, pyramid, plan): (&mut CameraCull, &mut Pyramid, &Plan),
        (depth, view_proj): (&wgpu::TextureView, glam::Mat4),
        (n, batches): (u32, u32),
        close: Option<wgpu::ComputePassTimestampWrites>,
    ) -> Option<ClusterDraws> {
        let clustered = !plan.runs.is_empty();
        if clustered {
            let at = (&*pyramid, cam.visible.buffer(), view_proj);
            self.ready_clusters(encoder, (&mut cam.clusters, plan), at);
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
                _ => self.record_clusters(pass, (clusters, plan)),
            }
            self.lap(lap_of(part));
        };
        let after = [GpuPart::OcclusionPyramid, GpuPart::OcclusionTest];
        let parts = after
            .into_iter()
            .chain(clustered.then_some(GpuPart::OcclusionClusters));
        let begin = |encoder: &mut wgpu::CommandEncoder, label, writes| {
            let desc = wgpu::ComputePassDescriptor {
                label: Some(label),
                timestamp_writes: writes,
            };
            encoder.counted_compute_pass(&desc).forget_lifetime()
        };
        if self.gpu_timer.splits() {
            for part in parts {
                let writes = self.gpu_timer.part_compute_writes(part);
                record(&mut begin(encoder, lap_of(part), writes), part);
            }
        } else {
            let mut pass = begin(encoder, "Hi-Z Cull", close);
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
        GpuPart::OcclusionClusters => "occlusion.clusters",
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
