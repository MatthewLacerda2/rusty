//! One camera's cluster cull (#1968): its buffers, kept per view and stack
//! place like the instance cull's, and the three dispatches per clustered batch.

use std::sync::Arc;

use glam::{Mat4, Vec4};

use super::layout::RUN_STRIDE;
use super::plan::Plan;
use super::ClusterDraws;
use crate::render::gpu::grow_buffer::GrowBuffer;
use crate::render::passes::hiz::counters::Counter;
use crate::render::passes::hiz::kept::Kept;
use crate::render::passes::hiz::pyramid::Pyramid;
use crate::render::Renderer;

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

/// What group 0 binds, by binding: kept while none of them was replaced.
type GroupKey = ([wgpu::Buffer; 7], wgpu::TextureView, wgpu::Buffer);

/// One stacked camera's cluster buffers.
pub(crate) struct ClusterCull {
    params: wgpu::Buffer,
    jobs: GrowBuffer,
    state: GrowBuffer,
    out_indices: GrowBuffer,
    out_args: GrowBuffer,
    runs: GrowBuffer,
    pub(crate) counter: Counter,
    group: Kept<GroupKey, wgpu::BindGroup>,
    /// The jobs its last cull ran.
    #[cfg(test)]
    pub(crate) ran: u32,
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
            state: buffer("Cluster State", U::STORAGE | U::COPY_SRC),
            out_indices: buffer("Cluster Indices", U::STORAGE | U::INDEX),
            out_args: buffer("Cluster Args", U::STORAGE | U::INDIRECT | U::COPY_SRC),
            runs: buffer("Cluster Runs", U::UNIFORM),
            counter: Counter::default(),
            group: Kept::default(),
            #[cfg(test)]
            ran: 0,
        }
    }

    /// The jobs' indirect args, for a test's read back.
    #[cfg(test)]
    pub(crate) fn args(&self) -> &wgpu::Buffer {
        self.out_args.buffer()
    }
}

/// The eye `hiz_clusters.wgsl` faces clusters against: `view_proj`'s centre of
/// projection, which `(0, 0, 1, 0)` in clip space maps back to — a point (`w` 1)
/// for a perspective camera, the view direction (`w` 0) for an orthographic one.
pub(super) fn eye_of(view_proj: Mat4) -> Vec4 {
    let e = view_proj.inverse() * Vec4::new(0.0, 0.0, 1.0, 0.0);
    if e.w.abs() > 1e-6 * e.truncate().length().max(1e-30) {
        (e.truncate() / e.w).extend(1.0)
    } else {
        e.truncate().normalize_or_zero().extend(0.0)
    }
}

impl Renderer {
    /// Record `plan`'s three dispatches per clustered batch into `encoder`,
    /// against `pyramid` and `visible` (this frame's instance verdicts), and
    /// return the draws that stand for those batches.
    pub(crate) fn cull_clusters(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        (cull, plan): (&mut ClusterCull, &Plan),
        (pyramid, visible, view_proj): (&Pyramid, &wgpu::Buffer, Mat4),
        batches: usize,
    ) -> ClusterDraws {
        self.upload_clusters(cull, plan, (pyramid, view_proj));
        let key = (
            [
                cull.params.clone(),
                cull.jobs.buffer().clone(),
                self.draw_buffers.objects().clone(),
                visible.clone(),
                cull.state.buffer().clone(),
                cull.out_indices.buffer().clone(),
                cull.out_args.buffer().clone(),
            ],
            pyramid.view.clone(),
            cull.runs.buffer().clone(),
        );
        let layout = &self.hiz.clusters.cull_layout;
        let device = &self.device;
        let mut kept = std::mem::take(&mut cull.group);
        kept.get(key, || {
            let buffers = cull_buffers(cull, self, visible);
            super::layout::cull_group(device, layout, &buffers, &pyramid.view)
        });
        cull.group = kept;
        let group = cull.group.value().expect("made above");
        encoder.clear_buffer(cull.state.buffer(), 0, Some(4));
        #[cfg(test)]
        {
            cull.ran = plan.jobs.len() as u32;
        }
        let pipes = &self.hiz.clusters;
        for (r, run) in plan.runs.iter().enumerate() {
            let mesh = self.gpu_mesh_of(run.mesh).and_then(|m| m.clusters.as_ref());
            let Some(mesh) = mesh.map(|c| &c.group) else {
                continue;
            };
            let offset = [r as u32 * RUN_STRIDE as u32];
            let jobs = run.jobs.len() as u32;
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_bind_group(0, group, &offset);
            pass.set_bind_group(1, mesh, &[]);
            pass.set_pipeline(pipes.test.get());
            pass.dispatch_workgroups((jobs * run.clusters).div_ceil(64), 1, 1);
            pass.set_pipeline(pipes.scan.get());
            pass.dispatch_workgroups(jobs.div_ceil(64), 1, 1);
            pass.set_pipeline(pipes.copy.get());
            pass.dispatch_workgroups(run.clusters, jobs, 1);
        }
        cull.counter
            .copy(&self.device, encoder, (cull.state.buffer(), 4));
        ClusterDraws {
            indices: cull.out_indices.buffer().clone(),
            args: cull.out_args.buffer().clone(),
            draws: Arc::new(plan.draws(batches)),
        }
    }

    /// Write `plan`'s jobs, runs and the camera's params into `cull`'s buffers.
    fn upload_clusters(
        &self,
        cull: &mut ClusterCull,
        plan: &Plan,
        (pyramid, view_proj): (&Pyramid, Mat4),
    ) {
        let (device, queue) = (&self.device, &self.queue);
        let stride = std::mem::size_of::<crate::render::gpu::uniforms::ObjectData>() / 16;
        let params = Params {
            view_proj: view_proj.to_cols_array(),
            eye: eye_of(view_proj).to_array(),
            size: [pyramid.depth.0 as f32, pyramid.depth.1 as f32],
            levels: pyramid.levels(),
            object_stride: stride as u32,
        };
        queue.write_buffer(&cull.params, 0, bytemuck::bytes_of(&params));
        cull.jobs
            .upload(device, queue, bytemuck::cast_slice(&plan.jobs));
        let mut runs = vec![0u32; plan.runs.len().max(1) * RUN_STRIDE as usize / 4];
        for (r, run) in plan.runs.iter().enumerate() {
            let at = r * RUN_STRIDE as usize / 4;
            runs[at..at + 3].copy_from_slice(&[
                run.jobs.start,
                run.jobs.len() as u32,
                run.clusters,
            ]);
        }
        cull.runs.upload(device, queue, bytemuck::cast_slice(&runs));
        // Word 0 is the count; the jobs' words follow (`Plan::states` counts it).
        cull.state.reserve(device, 4 * u64::from(plan.states));
        cull.out_indices
            .reserve(device, 4 * u64::from(plan.out_indices.max(1)));
        let args = crate::render::passes::hiz::ARGS * plan.jobs.len().max(1) as u64;
        cull.out_args.reserve(device, args);
    }
}

/// Group 0's buffers, by binding, the pyramid aside.
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
        cull.out_indices.buffer(),
        cull.out_args.buffer(),
        cull.runs.buffer(),
    ]
}

#[cfg(test)]
mod tests {
    use glam::camera::rh::{proj::directx, view::look_at_mat4};
    use glam::Vec3;

    use super::eye_of;

    #[test]
    fn the_eye_is_the_camera_for_a_perspective_and_its_direction_for_an_ortho() {
        let view = look_at_mat4(Vec3::new(1.0, 2.0, 3.0), Vec3::new(1.0, 2.0, -5.0), Vec3::Y);
        let persp = directx::perspective(1.0, 1.5, 0.1, 100.0) * view;
        let e = eye_of(persp);
        assert!(
            (e.truncate() - Vec3::new(1.0, 2.0, 3.0)).length() < 1e-3 && e.w == 1.0,
            "{e}"
        );
        let ortho = directx::orthographic(-2.0, 2.0, -2.0, 2.0, 0.1, 100.0) * view;
        let e = eye_of(ortho);
        assert!(
            (e.truncate() - Vec3::NEG_Z).length() < 1e-4 && e.w == 0.0,
            "{e}"
        );
    }
}
