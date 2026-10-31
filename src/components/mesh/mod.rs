//! src/components/mesh/ — Mesh component, its vertex layout and the built-in
//! primitive geometry (`vertex`, `primitives`).
//!
//! primitive/imported vertex+index data. Moved from the former `core/scene.rs`.
//!
//! A mesh says its geometry changed with a [`MeshRevision`], not a flag the
//! renderer clears (#1540): several renderers draw one scene (the editor's, and
//! each bake's headless one), and a flag cleared by the first hid the change from
//! the rest, so the editor kept drawing a re-imported mesh's old vertices.

use crate::asset::{AnimationClip, SkinData};
use serde::{Deserialize, Serialize};
use std::ops::Range;
use std::sync::atomic::{AtomicU64, Ordering};

pub mod primitives;
mod shadow_casting;
mod skeleton;
mod vertex;

pub use shadow_casting::ShadowCasting;
pub use skeleton::{BoneBinding, BoneBody};
pub use vertex::Vertex;

/// Which edit of a mesh's geometry its vertices are (#1540). Every rebuilt or new
/// geometry takes a fresh one, newer than any before it; a clone keeps it, since
/// it holds the same vertices. Each renderer remembers the revision it uploaded per
/// geometry and re-uploads when a mesh carries a newer one, so a change reaches
/// every renderer drawing the scene, however many there are. Never saved: a loaded
/// mesh is new geometry. The value is never read by the sim.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MeshRevision(u64);

/// The next revision handed out.
static NEXT_REVISION: AtomicU64 = AtomicU64::new(1);

impl MeshRevision {
    /// A revision newer than every one handed out before.
    pub fn next() -> Self {
        Self(NEXT_REVISION.fetch_add(1, Ordering::Relaxed))
    }
}

impl Default for MeshRevision {
    /// New geometry: a fresh revision.
    fn default() -> Self {
        Self::next()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeshComponent {
    /// How this mesh's geometry is rehydrated on load. `"Box"`/`"Sphere"`/
    /// `"Plane"`/`"Cylinder"`/`"Capsule"` rebuild a primitive; `"Asset"` re-imports an
    /// authored source file (see `asset_ref`). GPU buffers are never persisted.
    pub primitive_type: String,
    /// Path-based reference into an imported source file, `path::sub_object`
    /// (e.g. `assets/models/crates.glb::Barrel`). Present iff
    /// `primitive_type == "Asset"`; the scene layer re-imports it on load. This is
    /// the only identity stored — the `.meta` sidecar holds settings, never this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_ref: Option<String>,
    /// A `"Plane"`'s quads per side (#1524): 1, the single quad, up to
    /// [`primitives::MAX_PLANE_SUBDIVISIONS`]. A vertex block (`vertex_waves`) needs
    /// the vertices to move. Every other mesh keeps 1; saved only when it isn't 1.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub subdivisions: u32,
    /// Whether and how it casts shadows (#1571): Unity's *Cast Shadows*. Always
    /// written, so `Component.Get` reads `On` too; a file without it reads `On`.
    #[serde(default)]
    pub cast_shadows: ShadowCasting,
    #[serde(skip)]
    pub vertices: Vec<Vertex>,
    #[serde(skip)]
    pub indices: Vec<u32>,
    /// The sub-mesh runs of `indices` (#1285), in order: run `i` draws with the
    /// entity's material `i` (`MaterialComponent::materials`). Empty means one run
    /// over every index, as a primitive has. Rehydrated from the imported sub-mesh's
    /// material ranges, never serialized (like `indices`).
    #[serde(skip)]
    pub ranges: Vec<Range<u32>>,
    /// The bind-pose bone palette for a skinned `"Asset"` mesh, indexed by joint
    /// (matching the vertices' `joint_indices`). Empty for static/primitive meshes.
    /// Like `vertices`, it is never persisted — it is rehydrated from the imported
    /// skin on scene load (#79); the renderer uploads it to the GPU bone buffer.
    #[serde(skip)]
    pub bind_palette: Vec<glam::Mat4>,
    /// The imported skeleton for a skinned `"Asset"` mesh, used by the animation
    /// runtime (#80) to pose the joints. `None` for static/primitive meshes.
    /// Rehydrated from the source on load, never serialized (like `vertices`).
    #[serde(skip)]
    pub skin: Option<SkinData>,
    /// The animation clips that drive this mesh's skeleton, keyed by name from the
    /// source file (#80). Empty for unanimated/static meshes; rehydrated on load.
    #[serde(skip)]
    pub clips: Vec<AnimationClip>,
    /// The currently *posed* bone palette, rebuilt each frame from the bone
    /// GameObjects' transforms (#453) once the animator and every later writer
    /// (scripts, physics) have posed them. Empty before the skeleton is spawned;
    /// the renderer then falls back to `bind_palette` (the rest pose). Like the bind
    /// palette it is runtime-only and never serialized.
    #[serde(skip)]
    pub pose_palette: Vec<glam::Mat4>,
    /// The bone GameObjects this skin is driven by, plus the designer's per-bone
    /// overrides (#453). Only the overrides are saved, and only when there are any.
    #[serde(default, skip_serializing_if = "BoneBinding::is_unmodified")]
    pub skeleton: BoneBinding,
    /// Which edit of the geometry this is; set a [`MeshRevision::next`] whenever
    /// `vertices` or `indices` change, so every renderer re-uploads them.
    #[serde(skip)]
    pub revision: MeshRevision,
}

fn one() -> u32 {
    1
}

fn is_one(n: &u32) -> bool {
    *n == 1
}

impl MeshComponent {
    /// A static mesh: `primitive_type` (see the field) over this geometry, with no
    /// asset reference, skin or clips, and a fresh revision so every renderer uploads it.
    /// Set the skinned and asset fields on top with `..MeshComponent::new(…)`.
    pub fn new(
        primitive_type: impl Into<String>,
        vertices: Vec<Vertex>,
        indices: Vec<u32>,
    ) -> Self {
        Self {
            primitive_type: primitive_type.into(),
            asset_ref: None,
            subdivisions: 1,
            cast_shadows: ShadowCasting::On,
            vertices,
            indices,
            ranges: Vec::new(),
            bind_palette: Vec::new(),
            skin: None,
            clips: Vec::new(),
            pose_palette: Vec::new(),
            skeleton: BoneBinding::default(),
            revision: MeshRevision::next(),
        }
    }

    /// This mesh with its rehydrated, never-saved fields (every `#[serde(skip)]` one:
    /// geometry, skin, clips, palettes, the bone slots) cleared and every saved field
    /// copied: what a prefab override diff reads, without cloning a whole model every
    /// frame (#1527). It is the mesh as serde writes it, read back, so a saved field
    /// added later is copied with no change here.
    pub fn reference_only(&self) -> Self {
        serde_json::to_value(self)
            .and_then(serde_json::from_value)
            .unwrap_or_else(|_| Self::new(self.primitive_type.clone(), Vec::new(), Vec::new()))
    }

    /// The index runs to draw, one per material slot: `ranges`, or the whole index
    /// list as one run when it is empty.
    pub fn index_ranges(&self) -> impl Iterator<Item = Range<u32>> + '_ {
        let whole = self
            .ranges
            .is_empty()
            .then_some(0..self.indices.len() as u32);
        whole.into_iter().chain(self.ranges.iter().cloned())
    }

    /// The bone palette the renderer should upload: the live posed palette when an
    /// animation is driving the skeleton, otherwise the static bind pose.
    pub fn active_palette(&self) -> &[glam::Mat4] {
        if self.pose_palette.is_empty() {
            &self.bind_palette
        } else {
            &self.pose_palette
        }
    }

    /// Whether this mesh is skinned (carries an imported skeleton). Frustum culling
    /// bounds a skinned mesh by its posed skeleton, not its rest-pose AABB, which an
    /// animation can push a limb outside of (#833, `render::SkinBounds`).
    pub fn is_skinned(&self) -> bool {
        self.skin.is_some()
    }

    /// World-space AABB of the vertices under `world_matrix`, or `None` for an
    /// empty mesh. (Was `Entity::compute_world_aabb`; lives on the component so
    /// callers reach it through the #344 accessor facade.)
    pub fn world_aabb(&self, world_matrix: glam::Mat4) -> Option<(glam::Vec3, glam::Vec3)> {
        if self.vertices.is_empty() {
            return None;
        }
        let mut min = glam::Vec3::splat(f32::MAX);
        let mut max = glam::Vec3::splat(f32::MIN);
        for v in &self.vertices {
            let world_pos = world_matrix.transform_point3(glam::Vec3::from_array(v.position));
            min = min.min(world_pos);
            max = max.max(world_pos);
        }
        Some((min, max))
    }
}

/// Which material slot draws which of a mesh's index `runs` (#1285), Unity's way
/// (`Renderer.sharedMaterials`): slot `i` draws run `i`; a slot past the last run
/// draws the last run again, so an extra material layers over it; a run past the
/// last slot is not drawn. `slots` is `None` for an entity without a Material, whose
/// every run draws with the engine default. The renderer and the lightmap bake
/// (#1290) both pair slots with runs through this one rule.
pub fn material_runs(runs: Vec<Range<u32>>, slots: Option<usize>) -> Vec<(usize, Range<u32>)> {
    let Some(slots) = slots else {
        return runs.into_iter().enumerate().collect();
    };
    let Some(last) = runs.last() else {
        return Vec::new();
    };
    (0..slots)
        .map(|slot| (slot, runs.get(slot).unwrap_or(last).clone()))
        .collect()
}

/// `runs` clipped to the first `count` indices, empty runs dropped (#1285). A mesh's
/// runs always cover its own indices; the clip only matters when its geometry changed
/// ahead of a GPU upload. The renderer and the lightmap bake both clip through this
/// before [`material_runs`], so the two never pair slots with different runs.
pub fn clip_runs(runs: impl Iterator<Item = Range<u32>>, count: u32) -> Vec<Range<u32>> {
    runs.map(|r| r.start.min(count)..r.end.min(count))
        .filter(|r| !r.is_empty())
        .collect()
}

#[cfg(test)]
mod box_lightmap_tests;
#[cfg(test)]
mod material_runs_tests;
#[cfg(test)]
#[path = "primitives_tests.rs"]
mod primitives_tests;
#[cfg(test)]
mod revision_tests;
#[cfg(test)]
#[path = "winding_tests.rs"]
mod winding_tests;
