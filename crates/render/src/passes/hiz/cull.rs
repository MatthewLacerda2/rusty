//! One camera's GPU occlusion cull (#1963, the [module docs](super)): its buffers,
//! kept per view and stack place, and what is uploaded and bound before the
//! stage (`stage.rs`) records the two phases.

use super::clusters::plan::Plan;
use super::clusters::{ClusterCull, ClusterDraws};
use super::counters::{Counter, Triangles};
use super::drawn::Layout;
use super::pyramid::Pyramid;
use super::stage::{lap_of, HOLD_LAP};
use super::tables::Tables;
use super::upload::{pack_bounds, Frame, Params};
use crate::draw::resources::SolidResources;
use crate::gpu::draw_buffers::DRAW_GROUP_INSTANCES;
use crate::gpu::grow_buffer::GrowBuffer;
use crate::gpu::kept::Kept;
use crate::timing::GpuPart;

use crate::{RenderView, Renderer};

/// The opaque draws a camera's passes make through the GPU's verdict: the per-draw group
/// naming the kept instances, and one indirect draw per batch, in batch order,
/// from byte `at` of `args`.
#[derive(Clone)]
pub(crate) struct Culled {
    pub group: wgpu::BindGroup,
    pub args: wgpu::Buffer,
    pub at: u64,
    /// The draws standing for the clustered batches (#1968): the final draws'
    /// only, and only when a batch is drawn by cluster.
    pub clusters: Option<ClusterDraws>,
    /// The final draws' only: where in `args` the late set's draws start
    /// (#2336, [`Self::late`]).
    late: Option<u64>,
    /// The final draws' only: the triangles the camera's culled passes drew,
    /// as last read back ([`drawn`](super::drawn), #2387), a frame or so late.
    pub triangles: Triangles,
}

impl Culled {
    /// The final draws' late set (#2336): the instances visible this frame that
    /// phase one did not draw, each batch drawn whole, clustered or not (its
    /// depth is its kept clusters', the clusters culled adding no nearer texel).
    /// Drawn over phase one's depth it leaves the depth the final set's prepass
    /// would. `None` for phase one's draws.
    pub(crate) fn late(&self) -> Option<Culled> {
        let at = self.late?;
        Some(Culled {
            at,
            clusters: None,
            late: None,
            ..self.clone()
        })
    }
}

/// `DrawIndexedIndirectArgs`' size: five words.
pub(crate) const ARGS: u64 = 20;

/// Where the late set's args start in the final args, for `batches` batches:
/// after the final draws' (`hiz.wgsl`'s `compact`).
pub(super) fn late_args(batches: usize) -> u64 {
    ARGS * batches as u64
}

/// One stacked camera's buffers: what it tests, last frame's verdict by object
/// slot, and the two phases' kept instances and args (the final ones followed by
/// the late set's, #2336).
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
    /// How many instances its final draws and its late set (#2336) drew, one
    /// read back of both and of what they drew ([`Layout::read`]).
    pub(super) counter: Counter,
    /// Which batches the read in flight's draws drew ([`Layout`], #2387).
    pub(super) reading: Layout,
    /// Its per-cluster cull (#1968).
    pub(crate) clusters: ClusterCull,
    /// The two `hiz.wgsl` groups (phase one's, then the test's and final), kept
    /// while the buffers and pyramid they bind are.
    pub(super) groups: Kept<CullKey, [wgpu::BindGroup; 2]>,
    /// The two phases' draws, kept while the per-draw group's buffers they bind and the
    /// args they draw from are.
    culled: Kept<[CulledKey; 2], [Culled; 2]>,
    /// The CPU tables it fills each frame.
    pub(super) tables: Tables,
    /// The batches its last cull drew.
    #[cfg(test)]
    pub(crate) drawn_batches: u32,
}

/// What a phase's [`Culled`] names: its per-draw group's buffers, by binding, and its
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
            reading: Layout::default(),
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
    /// Make room in `view` for stacked camera `idx`, and a pyramid at its depth's
    /// size.
    pub(super) fn hold_hiz(&self, view: &mut RenderView, idx: usize) {
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
    pub(super) fn prepare_cull(
        &mut self,
        view: &mut RenderView,
        idx: usize,
        (solids, view_proj): (&SolidResources, glam::Mat4),
        n: u32,
    ) -> ([Culled; 2], Plan) {
        let cam = &mut view.hiz.cameras[idx];
        let pyramid = view.hiz.pyramid.as_ref().expect("held");
        let reading = &cam.reading;
        cam.counter
            .collect(&self.device, |words| reading.read(words));
        cam.clusters
            .counter
            .collect(&self.device, Counter::first_word);
        self.lap(HOLD_LAP);
        let instances = &solids.draws.instances[..n as usize];
        let tested = pack_bounds(&mut cam.tables, instances, &solids.tested);
        self.lap(lap_of(GpuPart::OcclusionTest));
        let plan = self.plan_clusters(&solids.draws, &cam.tables);
        self.lap(lap_of(GpuPart::OcclusionClusters));
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
            let draw_group = self.draw_buffers.draw_group_buffers(kept).map(Clone::clone);
            (draw_group, cam.args[k].buffer().clone())
        });
        // Its instances and args come from the key (#2021: the args grow on their
        // own, by batch, not by instance); the other per-draw group's buffers are read from
        // `draw_buffers` again, the same ones the key was built from just above.
        let make = |(draw_group, args): &CulledKey| Culled {
            group: self.draw_buffers.group_with_instances(
                &self.device,
                &self.entity_bones_layout,
                &draw_group[DRAW_GROUP_INSTANCES],
            ),
            args: args.clone(),
            at: 0,
            clusters: None,
            late: None,
            triangles: Triangles::default(),
        };
        let mut culled = std::mem::take(&mut cam.culled);
        let mut draws = culled.get(key.clone(), || key.each_ref().map(make)).clone();
        cam.culled = culled;
        draws[1].late = Some(late_args(solids.draws.opaque.len()));
        draws[1].triangles = cam.counter.triangles;
        (draws, plan)
    }
}
