//! One camera's GPU occlusion cull (#1963, the [module docs](super)): its buffers,
//! kept per view and stack place, and the stage that records the two phases.

use super::counters::Counter;
use super::pyramid::Pyramid;
use super::upload::{pack_bounds, Frame, Params};
use crate::render::draw::camera::CameraCtx;
use crate::render::draw::resources::SolidResources;
use crate::render::gpu::grow_buffer::GrowBuffer;
use crate::render::{RenderView, Renderer};

/// The opaque draws a camera's passes make through the GPU's verdict: the group 1
/// naming the kept instances, and one indirect draw per batch, in batch order.
#[derive(Clone)]
pub(crate) struct Culled {
    pub group: wgpu::BindGroup,
    pub args: wgpu::Buffer,
}

/// `DrawIndexedIndirectArgs`' size: five words.
pub(crate) const ARGS: u64 = 20;

/// One stacked camera's buffers: what it tests, last frame's verdict by object
/// slot, and the two phases' kept instances and args.
pub(crate) struct CameraCull {
    pub(super) params: wgpu::Buffer,
    pub(super) bounds: GrowBuffer,
    pub(super) batches: GrowBuffer,
    pub(super) history: GrowBuffer,
    /// How many object slots `history` holds.
    slots: u32,
    pub(super) visible: GrowBuffer,
    pub(super) kept: [GrowBuffer; 2],
    pub(super) args: [GrowBuffer; 2],
    pub(super) counter: Counter,
    /// This frame's two `hiz.wgsl` groups: phase one's, then the test's and final.
    groups: Vec<wgpu::BindGroup>,
    /// The batches its last cull drew.
    pub(crate) drawn_batches: u32,
}

impl CameraCull {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        use wgpu::BufferUsages as U;
        let buffer = |label, usage| GrowBuffer::new(device, label, U::STORAGE | usage);
        let indirect = U::INDIRECT | U::COPY_SRC;
        Self {
            params: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Hi-Z Params"),
                size: std::mem::size_of::<Params>() as u64,
                usage: U::UNIFORM | U::COPY_DST,
                mapped_at_creation: false,
            }),
            bounds: buffer("Hi-Z Bounds", U::empty()),
            batches: buffer("Hi-Z Batches", U::empty()),
            history: buffer("Hi-Z History", U::empty()),
            slots: 0,
            visible: buffer("Hi-Z Visible", U::empty()),
            kept: [0, 1].map(|_| buffer("Hi-Z Kept Instances", U::empty())),
            args: [0, 1].map(|_| buffer("Hi-Z Indirect Args", indirect)),
            counter: Counter::default(),
            groups: Vec::new(),
            drawn_batches: 0,
        }
    }

    /// The final args, for a test's read back.
    #[cfg(test)]
    pub(crate) fn final_args(&self) -> &wgpu::Buffer {
        self.args[1].buffer()
    }

    /// Make room for `slots` object slots of history; new ones read "drawn".
    pub(super) fn hold(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, slots: u32) {
        if slots <= self.slots {
            return;
        }
        let slots = slots.next_power_of_two();
        let drawn = vec![1u32; slots as usize];
        self.history
            .upload(device, queue, bytemuck::cast_slice(&drawn));
        self.slots = slots;
    }
}

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
        let [first, last] = self.prepare_cull(view, idx, (&ctx.solids, ctx.view_proj), n);
        let cam = &mut view.hiz.cameras[idx];
        let pyramid = view.hiz.pyramid.as_ref().expect("held above");
        let opaque = &ctx.solids.draws.opaque;
        let batches = opaque.len() as u32;
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.dispatch(&mut encoder, (&self.hiz.select, &cam.groups[0]), batches);
        self.update_global_bind_group();
        let phase_one = ("Hi-Z Phase One", None);
        let solids = (opaque.as_slice(), Some(&first));
        self.record_depth(&mut encoder, &view.depth_view, solids, phase_one);
        pyramid.build(&self.device, &self.hiz, &mut encoder, &view.depth_view);
        self.dispatch(&mut encoder, (&self.hiz.test, &cam.groups[1]), n);
        self.dispatch(&mut encoder, (&self.hiz.compact, &cam.groups[1]), batches);
        let args = (cam.args[1].buffer(), batches);
        cam.counter.copy(&self.device, &mut encoder, args);
        self.queue.submit(std::iter::once(encoder.finish()));
        cam.counter.request(n);
        self.frame_counters.hiz_culled += cam.counter.culled;
        ctx.solids.culled = Some(last);
    }

    /// Make room in `view` for stacked camera `idx`, and a pyramid at its depth's
    /// size.
    fn hold_hiz(&self, view: &mut RenderView, idx: usize) {
        while view.hiz.cameras.len() <= idx {
            view.hiz.cameras.push(CameraCull::new(&self.device));
        }
        let size = view.scene_size();
        let depth = (size.width, size.height);
        if view.hiz.pyramid.as_ref().is_none_or(|p| p.depth != depth) {
            view.hiz.pyramid = Some(Pyramid::new(&self.device, depth));
        }
    }

    /// Upload camera `idx`'s first `n` instances and their bounds, bind its
    /// groups, count what it tests, and return the two phases' draws.
    fn prepare_cull(
        &mut self,
        view: &mut RenderView,
        idx: usize,
        (solids, view_proj): (&SolidResources, glam::Mat4),
        n: u32,
    ) -> [Culled; 2] {
        let cam = &mut view.hiz.cameras[idx];
        let pyramid = view.hiz.pyramid.as_ref().expect("held");
        cam.counter.collect(&self.device);
        let instances = &solids.draws.instances[..n as usize];
        let slots: Vec<u32> = instances.iter().map(|i| i.object).collect();
        let bounds = pack_bounds(&slots, &solids.tested);
        let frame = Frame {
            view_proj,
            depth: pyramid.depth,
            levels: pyramid.levels(),
            batches: &solids.draws.opaque,
            bounds: &bounds,
            slots: slots.iter().max().map_or(1, |&s| s + 1),
        };
        cam.groups = self.upload_cull(cam, (&frame, &pyramid.view), n).to_vec();
        cam.drawn_batches = solids.draws.opaque.len() as u32;
        let tested = bounds.iter().filter(|b| b[3] > 0.5).count() as u32;
        self.frame_counters.hiz_tested += tested;
        [0, 1].map(|k| Culled {
            group: self.draw_buffers.group_with_instances(
                &self.device,
                &self.entity_bones_layout,
                cam.kept[k].buffer(),
            ),
            args: cam.args[k].buffer().clone(),
        })
    }

    /// One compute pass of `pipeline` over `group`, a thread per one of `count`.
    fn dispatch(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        (pipeline, group): (
            &crate::render::gpu::pipelines::LazyCompute,
            &wgpu::BindGroup,
        ),
        count: u32,
    ) {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(pipeline.get());
        pass.set_bind_group(0, group, &[]);
        pass.dispatch_workgroups(count.div_ceil(64), 1, 1);
    }
}
