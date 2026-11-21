mod opaque;
mod records;
pub(crate) mod sort;
mod traits;

pub(crate) use opaque::OpaqueBatches;
pub(crate) use sort::OpaqueOrder;
pub(crate) use traits::RecordTraits;

use std::ops::Range;

use crate::gpu::pipelines::Facing;
use crate::scene::{MaterialRecords, MeshHandle};
use crate::InstanceData;

/// What two solids must share to be drawn by one instanced call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct BatchKey {
    pub pipeline: usize,
    pub facing: Facing,
    pub mesh: MeshHandle,
    pub indices: IndexRun,
    pub material: usize,
    pub record: u32,
    pub traits: RecordTraits,
    pub bone_base: u32,
    pub depth_only: bool,
}

impl BatchKey {
    pub(crate) fn in_prepass(&self) -> bool {
        self.traits.lit
    }
}

pub(crate) struct DrawItem {
    pub key: BatchKey,
    pub instance: InstanceData,
    pub depth: f32,
    pub outline: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OutlineDraw {
    pub mesh: MeshHandle,
    pub facing: Facing,
    pub material: usize,
    pub indices: Range<u32>,
    pub uniform_slot: u32,
    pub instance: u32,
}

impl OutlineDraw {
    fn of(batch: &DrawBatch, instance: u32) -> Self {
        Self {
            mesh: batch.key.mesh,
            facing: batch.key.facing,
            material: batch.key.material,
            indices: batch.key.indices.range(),
            uniform_slot: batch.uniform_slot,
            instance,
        }
    }
}

#[derive(Clone)]
pub(crate) struct DrawBatch {
    pub key: BatchKey,
    pub uniform_slot: u32,
    pub instances: Range<u32>,
}

#[derive(Default)]
pub(crate) struct FrameDraws {
    pub instances: Vec<InstanceData>,
    pub opaque: OpaqueBatches,
    pub transparent: Vec<DrawBatch>,
    pub outline: Vec<OutlineDraw>,
}

impl FrameDraws {
    pub(crate) fn build(
        (mut opaque, mut transparent): (Vec<DrawItem>, Vec<DrawItem>),
        records: &MaterialRecords,
        (instancing, order): (bool, OpaqueOrder),
    ) -> Self {
        for item in opaque.iter_mut().chain(&mut transparent) {
            item.key.record = records.rep(item.key.record);
        }
        match instancing {
            true => opaque = sort::front_to_back(opaque, order),
            // Stable: the given order holds inside a rank.
            false => opaque.sort_by_key(|item| order.rank(&item.key)),
        }
        let mut out = Self::default();
        out.opaque = OpaqueBatches::new(out.push_runs(opaque, instancing, 0), order);
        let base = out.opaque.len() as u32;
        out.transparent = out.push_runs(transparent, instancing, base);
        out
    }

    fn push_runs(&mut self, items: Vec<DrawItem>, merge: bool, slot_base: u32) -> Vec<DrawBatch> {
        let mut batches: Vec<DrawBatch> = Vec::new();
        for item in items {
            let index = self.instances.len() as u32;
            match batches.last_mut() {
                Some(run) if merge && run.key == item.key => run.instances.end = index + 1,
                _ => batches.push(DrawBatch {
                    key: item.key,
                    uniform_slot: slot_base + batches.len() as u32,
                    instances: index..index + 1,
                }),
            }
            if item.outline {
                let run = batches.last().expect("the item joined or opened a run");
                self.outline.push(OutlineDraw::of(run, index));
            }
            self.instances.push(item.instance);
        }
        batches
    }

    pub(crate) fn batches(&self) -> impl Iterator<Item = &DrawBatch> {
        self.opaque.iter().chain(&self.transparent)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct IndexRun {
    pub start: u32,
    pub end: u32,
}

impl IndexRun {
    pub(crate) fn range(self) -> Range<u32> {
        self.start..self.end
    }

    pub(crate) fn len(self) -> u32 {
        self.end - self.start
    }
}

impl From<Range<u32>> for IndexRun {
    fn from(r: Range<u32>) -> Self {
        Self {
            start: r.start,
            end: r.end,
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod instancing_tests;

#[cfg(test)]
mod outline_tests;

#[cfg(test)]
mod order_tests;

#[cfg(test)]
mod primed_order_tests;
