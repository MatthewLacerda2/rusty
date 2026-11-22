use rusty_asset::mesh_data::Shared;
use rusty_asset::{AnimationClip, SkinData};
use serde::{Deserialize, Serialize};
use std::ops::Range;

mod data;
mod geometry;
pub mod primitives;
mod runs;
mod shadow_casting;
mod skeleton;

pub use data::{area_weighted_normals, MeshData, MAX_INDICES, MAX_VERTICES, SCENE_KIND};
pub use runs::{clip_runs, material_runs};
/// A test hook.
#[doc(hidden)]
pub use rusty_asset::mesh_data::shared::copies;
pub use rusty_asset::Vertex;
pub use shadow_casting::ShadowCasting;
pub use skeleton::{BoneBinding, BoneBody};

/// Which edit of a mesh's geometry its vertices are: defined with the
/// importer, which hands one out per import.
pub use rusty_asset::MeshRevision;

#[derive(Debug, Serialize, Deserialize)]
pub struct MeshComponent {
    pub primitive_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_ref: Option<String>,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub subdivisions: u32,
    /// Whether and how it casts shadows: Unity's *Cast Shadows*. Always
    /// written, so `Component.Get` reads `On` too; a file without it reads `On`.
    #[serde(default)]
    pub cast_shadows: ShadowCasting,
    #[serde(default = "one_f32")]
    pub scale_in_lightmap: f32,
    /// Shared with other instances: edit in place only through `vertices_mut`.
    #[serde(skip)]
    pub vertices: Shared<Vertex>,
    #[serde(skip)]
    pub indices: Shared<u32>,
    #[serde(skip)]
    pub ranges: Vec<Range<u32>>,
    #[serde(skip)]
    pub bind_palette: Vec<glam::Mat4>,
    #[serde(skip)]
    pub skin: Option<SkinData>,
    #[serde(skip)]
    pub clips: Vec<AnimationClip>,
    #[serde(skip)]
    pub pose_palette: Vec<glam::Mat4>,
    /// The bone GameObjects this skin is driven by, plus the designer's per-bone
    /// overrides. Only the overrides are saved, and only when there are any.
    #[serde(default, skip_serializing_if = "BoneBinding::is_unmodified")]
    pub skeleton: BoneBinding,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub max_draw_distance: f32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub min_screen_size: f32,
    #[serde(skip)]
    pub revision: MeshRevision,
    #[serde(skip)]
    pub own_geometry: Option<MeshRevision>,
    #[serde(skip)]
    pub clusters: Option<(
        MeshRevision,
        std::sync::Arc<rusty_asset::clusters::Clusters>,
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
    /// Unity has no upper bound; past a few, `max_map_edge` caps the map.
    pub const SCALE_IN_LIGHTMAP: std::ops::RangeInclusive<f32> = 0.0..=64.0;

    pub fn reads_probes(&self) -> bool {
        self.scale_in_lightmap <= 0.0
    }

    /// A test hook: `edit`'s and `script`'s import tests ask it.
    #[doc(hidden)]
    pub fn has_lightmap_uv(&self) -> bool {
        self.vertices.iter().any(|v| v.lightmap_uv != [0.0, 0.0])
    }

    pub fn new(
        primitive_type: impl Into<String>,
        vertices: impl Into<Shared<Vertex>>,
        indices: impl Into<Shared<u32>>,
    ) -> Self {
        Self {
            primitive_type: primitive_type.into(),
            asset_ref: None,
            subdivisions: 1,
            cast_shadows: ShadowCasting::On,
            scale_in_lightmap: 1.0,
            vertices: vertices.into(),
            indices: indices.into(),
            ranges: Vec::new(),
            bind_palette: Vec::new(),
            skin: None,
            clips: Vec::new(),
            pose_palette: Vec::new(),
            skeleton: BoneBinding::default(),
            max_draw_distance: 0.0,
            min_screen_size: 0.0,
            revision: MeshRevision::next(),
            own_geometry: None,
            clusters: None,
        }
    }

    pub fn clusters(&self) -> Option<&rusty_asset::clusters::Clusters> {
        let (of, clusters) = self.clusters.as_ref()?;
        (*of == self.revision).then_some(clusters)
    }

    pub fn reference_only(&self) -> Self {
        serde_json::to_value(self)
            .and_then(serde_json::from_value)
            .unwrap_or_else(|_| Self::new(self.primitive_type.clone(), Vec::new(), Vec::new()))
    }

    pub fn index_ranges(&self) -> impl Iterator<Item = Range<u32>> + '_ {
        let whole = self
            .ranges
            .is_empty()
            .then_some(0..self.indices.len() as u32);
        whole.into_iter().chain(self.ranges.iter().cloned())
    }

    pub fn active_palette(&self) -> &[glam::Mat4] {
        if self.pose_palette.is_empty() {
            &self.bind_palette
        } else {
            &self.pose_palette
        }
    }

    pub fn culls_when_small(&self) -> bool {
        self.max_draw_distance > 0.0 || self.min_screen_size > 0.0
    }

    pub fn is_skinned(&self) -> bool {
        self.skin.is_some()
    }

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

impl crate::FirstClass for MeshComponent {
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
