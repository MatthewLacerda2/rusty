//! Group one camera's solids into instanced draws (#470).
//!
//! Every visible solid becomes a [`DrawItem`]: its [`BatchKey`] (what must be equal for
//! two entities to share one draw call) and its [`InstanceData`] (what may differ).
//! [`FrameDraws::build`] turns the items into runs of equal keys — one draw call per
//! run — and packs every instance into one array in draw order, so the whole camera's
//! per-entity data is one buffer upload.
//!
//! Opaque items are sorted by key first, so every copy of a prop lands in one run,
//! then the runs are ordered roughly front to back ([`sort::front_to_back`], #1499),
//! so nearer solids fill the depth buffer before farther ones are shaded.
//! Transparent items are not: their back-to-front order is what makes blending
//! correct, so only *consecutive* equal keys merge there — "where order allows".
//! The selection outline (#1175) is recorded here too: an item marked `outline` gets
//! an [`OutlineDraw`] naming the very instance and uniform slot it was packed into.
//! Pure CPU logic, no GPU state, so the grouping rules are unit-tested without an
//! adapter.

mod records;
pub(crate) mod sort;

use std::ops::Range;

use crate::render::gpu::uniforms::MATERIAL_DATA_WORDS;
use crate::render::{InstanceData, MaterialData, MeshId};

/// What two solids must share to be drawn by one instanced call.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct BatchKey {
    /// The pipeline: the standard forward shader (0) or a material's authored surface
    /// shader (#396). First, so opaque draws sort into one run of state per shader.
    pub pipeline: usize,
    /// The geometry (vertex + index buffers), by asset identity (#127).
    pub mesh: MeshId,
    /// The drawn index run (#1285), whole: each material run of one mesh is a draw of
    /// its own, and two entities whose runs of one mesh are joined differently never
    /// share a draw.
    pub indices: IndexRun,
    /// The group-2 material bind group (the resolved texture maps), by cache index.
    pub material: usize,
    /// The material record's words (tint, PBR scalars, map and cutout flags, the
    /// surface and mapping terms, #1616), compared bit-for-bit.
    pub record: [u32; MATERIAL_DATA_WORDS],
    /// Where the draw's joint palette starts (#455): each skinned entity, with a
    /// palette of its own, is a draw of its own.
    pub bone_base: u32,
}

impl BatchKey {
    /// The material record this key carries.
    pub(crate) fn record(&self) -> MaterialData {
        MaterialData::from_words(&self.record)
    }

    /// Whether the SSAO depth prepass draws this batch, so its depth is in the prepass
    /// (#436): the one rule the prepass and depth priming (#1499) both read. Unlit
    /// solids (gizmos, a light's own mesh) cast no occlusion, so they are left out,
    /// and over a primed depth they write their own. Which fragments a drawn batch
    /// keeps is its shader's: `fs_prepass` / `fs_prepass_cut` clip Cutout texels, cut
    /// and move the UVs as `fs_main` does (`shadergen::assemble::depth`).
    pub(crate) fn in_prepass(&self) -> bool {
        self.record().is_lit != 0
    }
}

/// One visible solid, ready to batch.
pub(crate) struct DrawItem {
    pub key: BatchKey,
    pub instance: InstanceData,
    /// Its view depth (the centre of its world bounds along the camera's forward
    /// axis): the opaque pass's front-to-back key (#1499).
    pub depth: f32,
    /// Whether the selection outline rings this solid (#1175): it is then drawn a
    /// second time, from this same draw, into the selection mask.
    pub outline: bool,
}

/// The selection outline's draw of one solid (#1175): the forward draw's own mesh,
/// material group, uniform slot (its per-draw uniform and bone palette) and instance
/// (its world matrix), so the outline re-derives nothing the forward pass decided.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OutlineDraw {
    pub mesh: MeshId,
    pub material: usize,
    pub indices: Range<u32>,
    pub uniform_slot: u32,
    pub instance: u32,
}

impl OutlineDraw {
    /// The outline of `instance`, packed into `batch`.
    fn of(batch: &DrawBatch, instance: u32) -> Self {
        Self {
            mesh: batch.key.mesh.clone(),
            material: batch.key.material,
            indices: batch.key.indices.range(),
            uniform_slot: batch.uniform_slot,
            instance,
        }
    }
}

/// One draw call: `instances` of `key`'s mesh, reading the per-draw uniform in slot
/// `uniform_slot` of the frame's packed uniforms.
#[derive(Clone)]
pub(crate) struct DrawBatch {
    pub key: BatchKey,
    pub uniform_slot: u32,
    pub instances: Range<u32>,
}

/// One camera's solids as instanced draws: the packed instance array, then the opaque
/// and transparent batches that index into it, and the outline draws of the solids
/// marked for it.
#[derive(Default)]
pub(crate) struct FrameDraws {
    pub instances: Vec<InstanceData>,
    pub opaque: Vec<DrawBatch>,
    pub transparent: Vec<DrawBatch>,
    pub outline: Vec<OutlineDraw>,
}

impl FrameDraws {
    /// Batch `opaque` (sorted by key, its runs nearest first) and `transparent` (order kept — the caller sorted
    /// it back-to-front). With `instancing` off every item is its own draw, in the
    /// order given: the pre-#470 path, kept for equivalence tests and measurement.
    pub(crate) fn build(
        mut opaque: Vec<DrawItem>,
        transparent: Vec<DrawItem>,
        instancing: bool,
    ) -> Self {
        if instancing {
            opaque = sort::front_to_back(opaque);
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

    /// Opaque then transparent batches — the draw calls this camera issues.
    pub(crate) fn batches(&self) -> impl Iterator<Item = &DrawBatch> {
        self.opaque.iter().chain(&self.transparent)
    }
}

/// A run of a mesh's index buffer, `start..end`, as a key field (#1285): `Range`
/// is neither `Copy` nor `Ord`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct IndexRun {
    pub start: u32,
    pub end: u32,
}

impl IndexRun {
    /// The run as the range `draw_indexed` takes.
    pub(crate) fn range(self) -> Range<u32> {
        self.start..self.end
    }

    /// How many indices the run draws.
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
