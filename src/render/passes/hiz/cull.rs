//! One camera's GPU occlusion cull (#1963, the [module docs](super)): its buffers,
//! kept per view and stack place, and the stage that records the two phases.

use super::clusters::plan::Plan;
use super::clusters::{ClusterCull, ClusterDraws};
use super::counters::Counter;
use super::kept::{Kept, Tables};
use super::pyramid::Pyramid;
use super::upload::{pack_bounds, Frame, Params};
use crate::render::draw::camera::CameraCtx;
use crate::render::draw::resources::SolidResources;
use crate::render::gpu::draw_buffers::GROUP1_INSTANCES;
use crate::render::gpu::grow_buffer::GrowBuffer;
use crate::render::{RenderView, Renderer};

/// The opaque draws a camera's passes make through the GPU's verdict: the group 1
/// naming the kept instances, and one indirect draw per batch, in batch order.
#[derive(Clone)]
pub(crate) struct Culled {
    pub group: wgpu::BindGroup,
    pub args: wgpu::Buffer,
    /// The draws standing for the clustered batches (#1968): the final draws'
    /// only, and only when a batch is drawn by cluster.
    pub clusters: Option<ClusterDraws>,
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
    /// Its per-cluster cull (#1968).
    pub(crate) clusters: ClusterCull,
    /// The two `hiz.wgsl` groups (phase one's, then the test's and final), kept
    /// while the buffers and pyramid they bind are.
    pub(super) groups: Kept<CullKey, [wgpu::BindGroup; 2]>,
    /// The two phases' draws, kept while the group-1 buffers they bind and the
    /// args they draw from are.
    culled: Kept<[CulledKey; 2], [Culled; 2]>,
    /// The CPU tables it fills each frame.
    pub(super) tables: Tables,
    /// The batches its last cull drew.
    #[cfg(test)]
    pub(crate) drawn_batches: u32,
}

/// What a phase's [`Culled`] names: its group 1's buffers, by binding, and its
/// indirect args.
type CulledKey = ([wgpu::Buffer; 6], wgpu::Buffer);

/// What the cull groups bind: per phase, its buffers by binding, and the pyramid.
pub(super) type CullKey = ([[wgpu::Buffer; 8]; 2], wgpu::TextureView);

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
            history: buffer("Hi-Z History", U::COPY_SRC),
            slots: 0,
            visible: buffer("Hi-Z Visible", U::empty()),
            kept: [0, 1].map(|_| buffer("Hi-Z Kept Instances", U::empty())),
            args: [0, 1].map(|_| buffer("Hi-Z Indirect Args", indirect)),
            counter: Counter::default(),
            clusters: ClusterCull::new(device),
            groups: Kept::default(),
            culled: Kept::default(),
            tables: Tables::default(),
            #[cfg(test)]
            drawn_batches: 0,
        }
    }

    /// The final args, for a test's read back.
    #[cfg(test)]
    pub(crate) fn final_args(&self) -> &wgpu::Buffer {
        self.args[1].buffer()
    }

    /// Make room for `slots` object slots of history. The verdicts it holds are
    /// kept; only the new slots, `[old, slots)`, read "drawn", so a growth past a
    /// power of two (a spawn, a stream-in) costs phase one just the new objects.
    pub(super) fn hold(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, slots: u32) {
        if slots <= self.slots {
            return;
        }
        let slots = slots.next_power_of_two();
        self.history
            .reserve_kept(device, queue, 4 * u64::from(slots));
        let drawn = vec![1u32; (slots - self.slots) as usize];
        let at = 4 * u64::from(self.slots);
        self.history
            .write_at(queue, at, bytemuck::cast_slice(&drawn));
        self.slots = slots;
    }

    /// How many object slots its history holds.
    #[cfg(test)]
    pub(crate) fn slots(&self) -> u32 {
        self.slots
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
        let ([first, mut last], plan) =
            self.prepare_cull(view, idx, (&ctx.solids, ctx.view_proj), n);
        let (cam, pyramid) = (&mut view.hiz.cameras[idx], &mut view.hiz.pyramid);
        let pyramid = pyramid.as_mut().expect("held above");
        let groups = cam.groups.value().expect("bound by prepare_cull");
        let opaque = &ctx.solids.draws.opaque;
        let batches = opaque.len() as u32;
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.dispatch(&mut encoder, (&self.hiz.select, &groups[0]), batches);
        self.update_global_bind_group();
        let phase_one = ("Hi-Z Phase One", None);
        let solids = (opaque, Some(&first));
        self.record_depth(&mut encoder, &view.depth_view, solids, phase_one);
        pyramid.build(&self.device, &self.hiz, &mut encoder, &view.depth_view);
        self.dispatch(&mut encoder, (&self.hiz.test, &groups[1]), n);
        self.dispatch(&mut encoder, (&self.hiz.compact, &groups[1]), batches);
        if !plan.runs.is_empty() {
            let at = (&*pyramid, cam.visible.buffer(), ctx.view_proj);
            let cull = (&mut cam.clusters, &plan);
            last.clusters = Some(self.cull_clusters(&mut encoder, cull, at, batches as usize));
        }
        let args = (cam.args[1].buffer(), ARGS * u64::from(batches));
        cam.counter.copy(&self.device, &mut encoder, args);
        self.queue.submit(std::iter::once(encoder.finish()));
        cam.counter.request(n);
        cam.clusters.counter.request(plan.tested);
        let counters = &mut self.frame_counters;
        counters.hiz_culled += cam.counter.culled;
        counters.cluster_tested += plan.tested;
        counters.cluster_culled += cam.clusters.counter.while_active(!plan.runs.is_empty());
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
    /// groups, count what it tests, and return the two phases' draws and the
    /// batches it draws by cluster (#1968).
    fn prepare_cull(
        &mut self,
        view: &mut RenderView,
        idx: usize,
        (solids, view_proj): (&SolidResources, glam::Mat4),
        n: u32,
    ) -> ([Culled; 2], Plan) {
        let cam = &mut view.hiz.cameras[idx];
        let pyramid = view.hiz.pyramid.as_ref().expect("held");
        cam.counter.collect(&self.device, Counter::drawn_instances);
        cam.clusters
            .counter
            .collect(&self.device, Counter::first_word);
        let instances = &solids.draws.instances[..n as usize];
        let tested = pack_bounds(&mut cam.tables, instances, &solids.tested);
        let plan = self.plan_clusters(&solids.draws, &cam.tables);
        let frame = Frame {
            view_proj,
            depth: pyramid.depth,
            levels: pyramid.levels(),
            batches: &solids.draws.opaque,
            clustered: &plan.draws(solids.draws.opaque.len()),
            slots: cam.tables.slots.iter().max().map_or(1, |&s| s + 1),
        };
        self.upload_cull(cam, (&frame, &pyramid.view), n);
        #[cfg(test)]
        {
            cam.drawn_batches = solids.draws.opaque.len() as u32;
        }
        self.frame_counters.hiz_tested += tested;
        let key = [0, 1].map(|k| {
            let kept = cam.kept[k].buffer();
            let group1 = self.draw_buffers.group1_buffers(kept).map(Clone::clone);
            (group1, cam.args[k].buffer().clone())
        });
        // Its instances and args come from the key (#2021: the args grow on their
        // own, by batch, not by instance); the other group-1 buffers are read from
        // `draw_buffers` again, the same ones the key was built from just above.
        let make = |(group1, args): &CulledKey| Culled {
            group: self.draw_buffers.group_with_instances(
                &self.device,
                &self.entity_bones_layout,
                &group1[GROUP1_INSTANCES],
            ),
            args: args.clone(),
            clusters: None,
        };
        let mut culled = std::mem::take(&mut cam.culled);
        let draws = culled.get(key.clone(), || key.each_ref().map(make)).clone();
        cam.culled = culled;
        (draws, plan)
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
