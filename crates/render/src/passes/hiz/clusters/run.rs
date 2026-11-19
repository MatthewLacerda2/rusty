//! One camera's cluster cull (#1968): its buffers, kept per view and stack
//! place like the instance cull's, and the three dispatches per clustered batch,
//! recorded into the stage's one pass after phase one (#2327): the test, a
//! workgroup per instance's scan, and the copy, dispatched indirect from what the
//! scan kept (#2406).

use crate::gpu::ops::CountedQueue as _;
use std::sync::Arc;

use glam::Mat4;

use super::eye::eye_of;
use super::layout::RUN_STRIDE;
use super::plan::Plan;
use super::ClusterDraws;
use crate::gpu::grow_buffer::GrowBuffer;
use crate::gpu::kept::Kept;
use crate::passes::hiz::counters::Counter;
use crate::passes::hiz::pyramid::Pyramid;
use crate::timing::GpuPart;
use crate::Renderer;

/// `hiz_clusters.wgsl`'s `Params`.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    view_proj: [f32; 16],
    eye: [f32; 4],
    size: [f32; 2],
    levels: u32,
    object_stride: u32,
}

/// What the test's and scan's group 0 binds, by binding: kept while none of
/// them was replaced.
type GroupKey = ([wgpu::Buffer; 8], wgpu::TextureView);
/// What the copy's group 0 binds.
type CopyKey = [wgpu::Buffer; 4];
/// The words of a batch's copy dispatch in the tally (x, y, z).
const COPY_WORDS: u32 = 3;

/// One stacked camera's cluster buffers.
pub(crate) struct ClusterCull {
    params: wgpu::Buffer,
    jobs: GrowBuffer,
    state: GrowBuffer,
    tally: GrowBuffer,
    out_indices: GrowBuffer,
    out_args: GrowBuffer,
    runs: GrowBuffer,
    pub(crate) counter: Counter,
    group: Kept<GroupKey, wgpu::BindGroup>,
    copy_group: Kept<CopyKey, wgpu::BindGroup>,
    /// The jobs its last cull ran.
    #[cfg(test)]
    pub(crate) ran: u32,
    /// The clustered runs (batches) its last cull recorded.
    #[cfg(test)]
    pub(crate) runs_ran: u32,
}

impl ClusterCull {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        use wgpu::BufferUsages as U;
        let buffer = |label, usage| GrowBuffer::new(device, label, usage);
        let plain = |label, size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        Self {
            params: plain(
                "Cluster Cull Params",
                std::mem::size_of::<Params>() as u64,
                U::UNIFORM | U::COPY_DST,
            ),
            jobs: buffer("Cluster Jobs", U::STORAGE),
            state: buffer("Cluster State", U::STORAGE),
            tally: buffer("Cluster Tally", U::STORAGE | U::INDIRECT | U::COPY_SRC),
            out_indices: buffer("Cluster Indices", U::STORAGE | U::INDEX | U::COPY_SRC),
            out_args: buffer("Cluster Args", U::STORAGE | U::INDIRECT | U::COPY_SRC),
            runs: buffer("Cluster Runs", U::UNIFORM),
            counter: Counter::default(),
            group: Kept::default(),
            copy_group: Kept::default(),
            #[cfg(test)]
            ran: 0,
            #[cfg(test)]
            runs_ran: 0,
        }
    }

    /// The jobs' indirect args, for a test's read back.
    #[cfg(test)]
    pub(crate) fn args(&self) -> &wgpu::Buffer {
        self.out_args.buffer()
    }

    /// The output index list, for a test's read back.
    #[cfg(test)]
    pub(crate) fn indices(&self) -> &wgpu::Buffer {
        self.out_indices.buffer()
    }

    /// The count, then each batch's copy dispatch, for a test's read back.
    #[cfg(test)]
    pub(crate) fn tally(&self) -> &wgpu::Buffer {
        self.tally.buffer()
    }
}

impl Renderer {
    /// Ready `cull` for `plan` before the stage's merged pass opens: upload its
    /// jobs, params and tally (a zero count, and each batch's copy dispatch at
    /// no cluster yet), and bind its groups against `pyramid` and `visible`
    /// (this frame's instance verdicts).
    pub(crate) fn ready_clusters(
        &self,
        (cull, plan): (&mut ClusterCull, &Plan),
        (pyramid, visible, view_proj): (&Pyramid, &wgpu::Buffer, Mat4),
    ) {
        self.upload_clusters(cull, plan, (pyramid, view_proj));
        self.bind_clusters(cull, (pyramid, visible));
        #[cfg(test)]
        {
            cull.ran = plan.jobs.len() as u32;
            cull.runs_ran = plan.runs.len() as u32;
        }
    }

    /// Record `plan`'s dispatches of `steps` (the cluster cull's sub-parts, in
    /// order) per clustered batch into `pass`, each run's group 0 at its own
    /// dynamic offset; `ready_clusters` ran first. A batch's dispatches read only
    /// its own state, so a timed frame records every batch's test, then every
    /// scan, then every copy, a pass each.
    pub(crate) fn record_clusters(
        &self,
        pass: &mut wgpu::ComputePass,
        (cull, plan): (&ClusterCull, &Plan),
        steps: &[GpuPart],
    ) {
        let group = cull.group.value().expect("bound by ready_clusters");
        let copy_group = cull.copy_group.value().expect("bound by ready_clusters");
        let pipes = &self.hiz.clusters;
        for (r, run) in plan.runs.iter().enumerate() {
            let mesh = self.gpu_mesh_of(run.mesh).and_then(|m| m.clusters.as_ref());
            let mesh = &mesh
                .expect("planned this frame from a resident clustered mesh")
                .group;
            let offset = [r as u32 * RUN_STRIDE as u32];
            let jobs = run.jobs.len() as u32;
            pass.set_bind_group(1, mesh, &[]);
            for step in steps {
                match step {
                    GpuPart::OcclusionClustersTest => {
                        pass.set_bind_group(0, group, &offset);
                        pass.set_pipeline(pipes.test.get());
                        pass.dispatch_workgroups((jobs * run.clusters).div_ceil(64), 1, 1);
                    }
                    GpuPart::OcclusionClustersScan => {
                        pass.set_bind_group(0, group, &offset);
                        pass.set_pipeline(pipes.scan.get());
                        pass.dispatch_workgroups(jobs, 1, 1);
                    }
                    GpuPart::OcclusionClustersCopy => {
                        // Sized by what the scan kept: no workgroup a culled cluster.
                        pass.set_bind_group(0, copy_group, &offset);
                        pass.set_pipeline(pipes.copy.get());
                        let at = 4 * u64::from(copy_at(r));
                        pass.dispatch_workgroups_indirect(cull.tally.buffer(), at);
                    }
                    other => unreachable!("{other:?} is no cluster cull dispatch"),
                }
            }
        }
    }

    /// After the merged pass ends: copy `cull`'s count for its read back, and
    /// return the draws that stand for `plan`'s batches among `batches`.
    pub(crate) fn clusters_drawn(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        (cull, plan): (&mut ClusterCull, &Plan),
        batches: usize,
    ) -> ClusterDraws {
        cull.counter
            .copy(&self.device, encoder, (cull.tally.buffer(), 4));
        ClusterDraws {
            indices: cull.out_indices.buffer().clone(),
            args: cull.out_args.buffer().clone(),
            draws: Arc::new(plan.draws(batches)),
        }
    }

    /// Bind `cull`'s two group 0s, each rebuilt only when a buffer or the
    /// pyramid it names was replaced.
    fn bind_clusters(&self, cull: &mut ClusterCull, (pyramid, visible): (&Pyramid, &wgpu::Buffer)) {
        let buffers = cull_buffers(cull, self, visible).map(Clone::clone);
        let copy = [&cull.jobs, &cull.state, &cull.runs, &cull.out_indices];
        let copy = copy.map(|b| b.buffer().clone());
        let pipes = &self.hiz.clusters;
        let device = &self.device;
        let view = &pyramid.view;
        cull.group.get((buffers.clone(), view.clone()), || {
            let at = (&[0, 1, 2, 3, 4, 5, 6, 8][..], &buffers.each_ref()[..]);
            super::layout::cull_group(device, &pipes.cull_layout, at, Some(view))
        });
        cull.copy_group.get(copy.clone(), || {
            let at = (&[1, 4, 8, 9][..], &copy.each_ref()[..]);
            super::layout::cull_group(device, &pipes.copy_layout, at, None)
        });
    }

    /// Write `plan`'s jobs, runs and the camera's params into `cull`'s buffers.
    fn upload_clusters(
        &self,
        cull: &mut ClusterCull,
        plan: &Plan,
        (pyramid, view_proj): (&Pyramid, Mat4),
    ) {
        let (device, queue) = (&self.device, &self.queue);
        let stride = std::mem::size_of::<crate::gpu::uniforms::ObjectData>() / 16;
        let params = Params {
            view_proj: view_proj.to_cols_array(),
            eye: eye_of(view_proj).to_array(),
            size: [pyramid.depth.0 as f32, pyramid.depth.1 as f32],
            levels: pyramid.levels(),
            object_stride: stride as u32,
        };
        queue.counted_write_buffer(&cull.params, 0, bytemuck::bytes_of(&params));
        cull.jobs
            .upload(device, queue, bytemuck::cast_slice(&plan.jobs));
        let mut runs = vec![0u32; plan.runs.len().max(1) * RUN_STRIDE as usize / 4];
        for (r, run) in plan.runs.iter().enumerate() {
            let at = r * RUN_STRIDE as usize / 4;
            runs[at..at + 4].copy_from_slice(&[
                run.jobs.start,
                run.jobs.len() as u32,
                run.clusters,
                copy_at(r),
            ]);
        }
        cull.runs.upload(device, queue, bytemuck::cast_slice(&runs));
        // The count, then each batch's copy dispatch: no cluster yet, its jobs.
        let mut tally = vec![0u32];
        for run in &plan.runs {
            tally.extend([0, run.jobs.len() as u32, 1]);
        }
        cull.tally
            .upload(device, queue, bytemuck::cast_slice(&tally));
        cull.state
            .reserve(device, 4 * u64::from(plan.states.max(1)));
        cull.out_indices
            .reserve(device, 4 * u64::from(plan.out_indices.max(1)));
        let args = crate::passes::hiz::ARGS * plan.jobs.len().max(1) as u64;
        cull.out_args.reserve(device, args);
    }
}

/// The word of the tally where batch `r`'s copy dispatch starts: after the count.
fn copy_at(r: usize) -> u32 {
    1 + COPY_WORDS * r as u32
}

/// The test's and scan's group 0 buffers, by binding, the pyramid aside.
fn cull_buffers<'a>(
    cull: &'a ClusterCull,
    r: &'a Renderer,
    visible: &'a wgpu::Buffer,
) -> [&'a wgpu::Buffer; 8] {
    [
        &cull.params,
        cull.jobs.buffer(),
        r.draw_buffers.objects(),
        visible,
        cull.state.buffer(),
        cull.tally.buffer(),
        cull.out_args.buffer(),
        cull.runs.buffer(),
    ]
}
