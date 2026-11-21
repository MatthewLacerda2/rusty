//! One camera's cluster cull (#1968): its buffers, kept per view and stack
//! place like the instance cull's, readied before the stage's one pass after
//! phase one (#2327) and read after it; `record` records its three stages
//! between, one dispatch each for the frame (#2417): the test and the copy over
//! the plan's chunks, the scan over its jobs.

use crate::gpu::ops::CountedQueue as _;
use std::sync::Arc;

use glam::Mat4;

use super::eye::eye_of;
use super::plan::Plan;
use super::ClusterDraws;
use crate::gpu::grow_buffer::GrowBuffer;
use crate::gpu::kept::Kept;
use crate::passes::hiz::counters::Counter;
use crate::passes::hiz::pyramid::Pyramid;
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
    jobs: u32,
    chunks: u32,
    _pad: [u32; 2],
}

/// What a stage's group 0 binds besides the params, by binding (the test's
/// pyramid aside): kept while none of them was replaced.
type GroupKey<const N: usize> = ([wgpu::Buffer; N], Option<wgpu::TextureView>);

/// One stacked camera's cluster buffers.
pub(crate) struct ClusterCull {
    params: wgpu::Buffer,
    jobs: GrowBuffer,
    chunks: GrowBuffer,
    state: GrowBuffer,
    /// The tested clusters kept, for `Debug.Stats`.
    pub(super) tally: GrowBuffer,
    out_indices: GrowBuffer,
    out_args: GrowBuffer,
    pub(crate) counter: Counter,
    pub(super) test_group: Kept<GroupKey<6>, wgpu::BindGroup>,
    pub(super) scan_group: Kept<GroupKey<3>, wgpu::BindGroup>,
    pub(super) copy_group: Kept<GroupKey<4>, wgpu::BindGroup>,
    /// The jobs its last cull ran.
    #[cfg(test)]
    pub(crate) ran: u32,
    /// The clustered runs (batches) its last cull recorded.
    #[cfg(test)]
    pub(crate) runs_ran: u32,
    /// The chunks its last cull's test and copy ran over.
    #[cfg(test)]
    pub(crate) chunks_ran: u32,
    /// The stages its last cull dispatched, in order, with each dispatch's
    /// workgroups (x, y).
    #[cfg(test)]
    pub(crate) dispatched: std::cell::RefCell<Vec<(crate::timing::GpuPart, (u32, u32))>>,
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
            chunks: buffer("Cluster Chunks", U::STORAGE),
            state: buffer("Cluster State", U::STORAGE),
            tally: buffer("Cluster Tally", U::STORAGE | U::COPY_SRC),
            out_indices: buffer("Cluster Indices", U::STORAGE | U::INDEX | U::COPY_SRC),
            out_args: buffer("Cluster Args", U::STORAGE | U::INDIRECT | U::COPY_SRC),
            counter: Counter::default(),
            test_group: Kept::default(),
            scan_group: Kept::default(),
            copy_group: Kept::default(),
            #[cfg(test)]
            ran: 0,
            #[cfg(test)]
            runs_ran: 0,
            #[cfg(test)]
            chunks_ran: 0,
            #[cfg(test)]
            dispatched: Default::default(),
        }
    }

    /// The jobs' indirect args: the per-instance draws' (#2387's read back).
    pub(crate) fn args(&self) -> &wgpu::Buffer {
        self.out_args.buffer()
    }

    /// The output index list, for a test's read back.
    #[cfg(test)]
    pub(crate) fn indices(&self) -> &wgpu::Buffer {
        self.out_indices.buffer()
    }

    /// The tested clusters kept, for a test's read back.
    #[cfg(test)]
    pub(crate) fn tally(&self) -> &wgpu::Buffer {
        self.tally.buffer()
    }
}

impl Renderer {
    /// Ready `cull` for `plan` before the stage's merged pass opens: upload its
    /// jobs, chunks, params and tally (a zero count), and bind its stages' groups
    /// against `pyramid` and `visible` (this frame's instance verdicts).
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
            cull.chunks_ran = plan.chunks.len() as u32;
            cull.dispatched.borrow_mut().clear();
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
            .copy(&self.device, encoder, &[(cull.tally.buffer(), 0, 4)]);
        ClusterDraws {
            indices: cull.out_indices.buffer().clone(),
            args: cull.out_args.buffer().clone(),
            draws: Arc::new(plan.draws(batches)),
        }
    }

    /// Bind `cull`'s three group 0s, each rebuilt only when a buffer or the
    /// pyramid it names was replaced.
    fn bind_clusters(&self, cull: &mut ClusterCull, (pyramid, visible): (&Pyramid, &wgpu::Buffer)) {
        use super::layout::{stage_group, COPY, SCAN, TEST};
        let pipes = &self.hiz.clusters;
        let (device, params) = (&self.device, &cull.params);
        let [jobs, chunks, state] = [&cull.jobs, &cull.chunks, &cull.state].map(GrowBuffer::buffer);
        let [tally, args, indices] =
            [&cull.tally, &cull.out_args, &cull.out_indices].map(GrowBuffer::buffer);
        let objects = self.draw_buffers.objects();
        let test = [jobs, objects, visible, state, tally, chunks].map(Clone::clone);
        let view = Some(pyramid.view.clone());
        cull.test_group.get((test.clone(), view.clone()), || {
            let at = (params, &TEST[..], &test[..]);
            stage_group(device, &pipes.test_layout, at, view.as_ref())
        });
        let scan = [jobs, state, args].map(Clone::clone);
        cull.scan_group.get((scan.clone(), None), || {
            stage_group(
                device,
                &pipes.scan_layout,
                (params, &SCAN[..], &scan[..]),
                None,
            )
        });
        let copy = [jobs, state, indices, chunks].map(Clone::clone);
        cull.copy_group.get((copy.clone(), None), || {
            stage_group(
                device,
                &pipes.copy_layout,
                (params, &COPY[..], &copy[..]),
                None,
            )
        });
    }

    /// Write `plan`'s jobs, chunks and the camera's params into `cull`'s buffers.
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
            jobs: plan.jobs.len() as u32,
            chunks: plan.chunks.len() as u32,
            _pad: [0; 2],
        };
        queue.counted_write_buffer(&cull.params, 0, bytemuck::bytes_of(&params));
        cull.jobs
            .upload(device, queue, bytemuck::cast_slice(&plan.jobs));
        cull.chunks
            .upload(device, queue, bytemuck::cast_slice(&plan.chunks));
        cull.tally.upload(device, queue, bytemuck::bytes_of(&0u32));
        cull.state
            .reserve(device, 4 * u64::from(plan.states.max(1)));
        cull.out_indices
            .reserve(device, 4 * u64::from(plan.out_indices.max(1)));
        let args = crate::passes::hiz::ARGS * plan.jobs.len().max(1) as u64;
        cull.out_args.reserve(device, args);
    }
}
