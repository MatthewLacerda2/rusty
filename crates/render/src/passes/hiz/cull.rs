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

#[derive(Clone)]
pub(crate) struct Culled {
    pub group: wgpu::BindGroup,
    pub args: wgpu::Buffer,
    pub at: u64,
    pub clusters: Option<ClusterDraws>,
    late: Option<u64>,
    pub triangles: Option<Triangles>,
}

impl Culled {
    /// Each late batch is drawn whole, clustered or not: a culled cluster adds no nearer texel.
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

/// Bytes: one `DrawIndexedIndirectArgs`, five `u32`s.
pub(crate) const ARGS: u64 = 20;

pub(super) fn late_args(batches: usize) -> u64 {
    ARGS * batches as u64
}

pub(crate) struct CameraCull {
    pub(super) params: wgpu::Buffer,
    pub(super) bounds: GrowBuffer,
    pub(super) batches: GrowBuffer,
    pub(super) history: GrowBuffer,
    slots: u32,
    pub(super) visible: GrowBuffer,
    pub(super) kept: [GrowBuffer; 2],
    pub(super) args: [GrowBuffer; 2],
    pub(super) counter: Counter,
    pub(super) reading: Layout,
    pub(crate) clusters: ClusterCull,
    pub(super) groups: Kept<CullKey, [wgpu::BindGroup; 2]>,
    culled: Kept<[CulledKey; 2], [Culled; 2]>,
    pub(super) tables: Tables,
    #[cfg(test)]
    pub(crate) drawn_batches: u32,
}

type CulledKey = ([wgpu::Buffer; 6], wgpu::Buffer);

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

    #[cfg(test)]
    pub(crate) fn final_args(&self) -> &wgpu::Buffer {
        self.args[1].buffer()
    }

    fn collect_reads(&mut self, device: &wgpu::Device) {
        let reading = &self.reading;
        self.counter.collect(device, |words| reading.read(words));
        let clusters = &mut self.clusters.counter;
        clusters.collect(device, Counter::first_word);
    }

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

    #[cfg(test)]
    pub(crate) fn slots(&self) -> u32 {
        self.slots
    }
}

impl Renderer {
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

    pub(super) fn prepare_cull(
        &mut self,
        view: &mut RenderView,
        idx: usize,
        (solids, view_proj): (&SolidResources, glam::Mat4),
        n: u32,
    ) -> ([Culled; 2], Plan) {
        let cam = &mut view.hiz.cameras[idx];
        let pyramid = view.hiz.pyramid.as_ref().expect("held");
        cam.collect_reads(&self.device);
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
        // The args are in the key: they grow by batch, not by instance.
        let key = [0, 1].map(|k| {
            let kept = cam.kept[k].buffer();
            let draw_group = self.draw_buffers.draw_group_buffers(kept).map(Clone::clone);
            (draw_group, cam.args[k].buffer().clone())
        });
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
            triangles: None,
        };
        let mut culled = std::mem::take(&mut cam.culled);
        let mut draws = culled.get(key.clone(), || key.each_ref().map(make)).clone();
        cam.culled = culled;
        draws[1].late = Some(late_args(solids.draws.opaque.len()));
        draws[1].triangles = cam.counter.triangles;
        (draws, plan)
    }
}
