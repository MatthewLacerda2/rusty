//! src/render/draw/batch.rs — group one camera's solids into instanced draws (#470).
//!
//! Every visible solid becomes a [`DrawItem`]: its [`BatchKey`] (what must be equal for
//! two entities to share one draw call) and its [`InstanceData`] (what may differ).
//! [`FrameDraws::build`] turns the items into runs of equal keys — one draw call per
//! run — and packs every instance into one array in draw order, so the whole camera's
//! per-entity data is one buffer upload.
//!
//! Opaque items are sorted by key first, so every copy of a prop lands in one run.
//! Transparent items are not: their back-to-front order is what makes blending
//! correct, so only *consecutive* equal keys merge there — "where order allows".
//! Pure CPU logic, no GPU state, so the grouping rules are unit-tested without an
//! adapter.

use std::ops::Range;

use crate::render::{EntityUniform, InstanceData, MeshId};

/// What two solids must share to be drawn by one instanced call.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct BatchKey {
    /// The geometry (vertex + index buffers), by asset identity (#127).
    pub mesh: MeshId,
    /// The group-2 material bind group (the resolved texture maps), by cache index.
    pub material: usize,
    /// The bone-palette slot: `0` is the shared identity palette every non-skinned mesh
    /// binds; a skinned entity gets a slot of its own, so it never shares a draw.
    pub bones: u32,
    /// The per-draw uniform's words (tint, PBR scalars, map and cutout flags),
    /// compared bit-for-bit.
    pub uniform: [u32; 36],
}

impl BatchKey {
    /// The per-draw uniform this key carries.
    pub(crate) fn uniform(&self) -> EntityUniform {
        bytemuck::pod_read_unaligned(bytemuck::cast_slice(&self.uniform))
    }
}

/// One visible solid, ready to batch.
pub(crate) struct DrawItem {
    pub key: BatchKey,
    pub num_indices: u32,
    pub instance: InstanceData,
}

/// One draw call: `instances` of `key`'s mesh, reading the per-draw uniform in slot
/// `uniform_slot` of the frame's packed uniforms.
pub(crate) struct DrawBatch {
    pub key: BatchKey,
    pub num_indices: u32,
    pub uniform_slot: u32,
    pub instances: Range<u32>,
}

/// One camera's solids as instanced draws: the packed instance array, then the opaque
/// and transparent batches that index into it.
#[derive(Default)]
pub(crate) struct FrameDraws {
    pub instances: Vec<InstanceData>,
    pub opaque: Vec<DrawBatch>,
    pub transparent: Vec<DrawBatch>,
}

impl FrameDraws {
    /// Batch `opaque` (sorted by key) and `transparent` (order kept — the caller sorted
    /// it back-to-front). With `instancing` off every item is its own draw, in the
    /// order given: the pre-#470 path, kept for equivalence tests and measurement.
    pub(crate) fn build(
        mut opaque: Vec<DrawItem>,
        transparent: Vec<DrawItem>,
        instancing: bool,
    ) -> Self {
        if instancing {
            // Stable, so copies of one prop keep their scene order inside the run.
            opaque.sort_by(|a, b| a.key.cmp(&b.key));
        }
        let mut out = Self::default();
        out.opaque = out.push_runs(opaque, instancing, 0);
        let base = out.opaque.len() as u32;
        out.transparent = out.push_runs(transparent, instancing, base);
        out
    }

    /// Append `items`' instances and return their batches, merging consecutive equal
    /// keys when `merge`. Uniform slots continue from `slot_base`.
    fn push_runs(&mut self, items: Vec<DrawItem>, merge: bool, slot_base: u32) -> Vec<DrawBatch> {
        let mut batches: Vec<DrawBatch> = Vec::new();
        for item in items {
            let index = self.instances.len() as u32;
            self.instances.push(item.instance);
            match batches.last_mut() {
                Some(run) if merge && run.key == item.key => run.instances.end = index + 1,
                _ => batches.push(DrawBatch {
                    key: item.key,
                    num_indices: item.num_indices,
                    uniform_slot: slot_base + batches.len() as u32,
                    instances: index..index + 1,
                }),
            }
        }
        batches
    }

    /// Every batch's per-draw uniform, in `uniform_slot` order.
    pub(crate) fn uniforms(&self) -> Vec<EntityUniform> {
        self.batches().map(|b| b.key.uniform()).collect()
    }

    /// Opaque then transparent batches — the draw calls this camera issues.
    pub(crate) fn batches(&self) -> impl Iterator<Item = &DrawBatch> {
        self.opaque.iter().chain(&self.transparent)
    }
}

#[cfg(test)]
#[path = "batch_tests.rs"]
mod batch_tests;
