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

mod geometry;
pub mod primitives;
mod shadow_casting;
mod skeleton;
mod vertex;

pub use shadow_casting::ShadowCasting;
pub use skeleton::{BoneBinding, BoneBody};
pub use vertex::Vertex;

/// Which edit of a mesh's geometry its vertices are (#1540): defined with the
/// importer, which hands one out per import (#857).
pub use crate::asset::MeshRevision;

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
    /// Unity's *Scale In Lightmap* (#1704): a multiplier on the bake's texel
    /// density for this mesh, in [`MeshComponent::SCALE_IN_LIGHTMAP`]. At 0 the
    /// mesh gets no lightmap and reads light probes ([`MeshComponent::reads_probes`]),
    /// while still blocking and bouncing light in the bake. Always written; a file
    /// without it reads 1.
    #[serde(default = "one_f32")]
    pub scale_in_lightmap: f32,
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
    /// Past this many metres from a camera (to its bounds' centre) the mesh is not
    /// drawn (#1714): Unreal's *Max Draw Distance*. `0`, the default, is unlimited.
    /// Saved only when set. `render::SmallObjectCull` reads it.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub max_draw_distance: f32,
    /// Below this screen-relative height (`0..1`: its local bounds' largest extent
    /// times its largest axis scale, over the view's height at its distance, the measure Unity's `LODGroup` picks its
    /// levels and its *Culled* share by) the mesh is not drawn (#1714): Unreal's
    /// min screen radius. `0`, the default, is off. Saved only when set.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub min_screen_size: f32,
    /// Which edit of the geometry this is; set a [`MeshRevision::next`] whenever
    /// `vertices` or `indices` change, so every renderer re-uploads them.
    #[serde(skip)]
    pub revision: MeshRevision,
    /// The clusters its import built (#1968, `asset::clusters`), with the
    /// revision they describe: any later edit takes a new revision, so stale
    /// clusters are never read. Read through [`Self::clusters`]; rehydrated with the
    /// geometry, never serialized.
    #[serde(skip)]
    pub clusters: Option<(
        MeshRevision,
        std::sync::Arc<crate::asset::clusters::Clusters>,
    )>,
}

fn one() -> u32 {
    1
}

fn one_f32() -> f32 {
    1.0
}

fn is_one(n: &u32) -> bool {
    *n == 1
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

impl MeshComponent {
    /// The Scale In Lightmap values a mesh takes. Unity has no upper bound; past a
    /// few the map reaches the page's cap anyway (`lightmap::max_map_edge`).
    pub const SCALE_IN_LIGHTMAP: std::ops::RangeInclusive<f32> = 0.0..=64.0;

    /// Whether a static mesh reads light probes instead of a lightmap: its Scale
    /// In Lightmap is 0, so the bake gives it none (#1704).
    pub fn reads_probes(&self) -> bool {
        self.scale_in_lightmap <= 0.0
    }

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
            scale_in_lightmap: 1.0,
            vertices,
            indices,
            ranges: Vec::new(),
            bind_palette: Vec::new(),
            skin: None,
            clips: Vec::new(),
            pose_palette: Vec::new(),
            skeleton: BoneBinding::default(),
            max_draw_distance: 0.0,
            min_screen_size: 0.0,
            revision: MeshRevision::next(),
            clusters: None,
        }
    }

    /// Its import's clusters (#1968), when they still describe its geometry: none
    /// once the vertices or indices were edited.
    pub fn clusters(&self) -> Option<&crate::asset::clusters::Clusters> {
        let (of, clusters) = self.clusters.as_ref()?;
        (*of == self.revision).then_some(clusters)
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

    /// Whether a camera may cull this mesh for its distance or its size on screen
    /// (#1714): either of `max_draw_distance` and `min_screen_size` is set. Such a
    /// mesh's shadow, and every renderer's of a `LODGroup` holding one, is drawn
    /// every frame with the camera's verdict, never baked into a cached static
    /// shadow (`render::passes::shadows::UnbakedCasters`).
    pub fn culls_when_small(&self) -> bool {
        self.max_draw_distance > 0.0 || self.min_screen_size > 0.0
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

impl crate::components::FirstClass for MeshComponent {
    const NAME: &'static str = "Mesh";
    const ALIASES: &'static [&'static str] = &["meshfilter", "meshrenderer"];
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
