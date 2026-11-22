//! stored-format: the scene document (`SceneData`), its serde shape and defaults

use std::collections::BTreeMap;

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::collision_matrix::CollisionMatrix;
use crate::layers::LayerRegistry;
use crate::lighting::lightmap_set::LightmapSet;
use crate::lighting::LightingSettings;
use crate::nav_settings::NavMeshSettings;
use rusty_components::{Entity, MaterialAsset};

pub use super::rehydrate::{asset_mesh_component, asset_ref_file, RehydratedScene};

fn default_skybox_path() -> String {
    String::new()
}
fn default_ambient_color() -> Vec3 {
    crate::DEFAULT_AMBIENT_COLOR
}
fn default_ambient_intensity() -> f32 {
    crate::DEFAULT_AMBIENT_INTENSITY
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SceneData {
    pub entities: Vec<Entity>,
    pub next_entity_id: u32,
    pub selected_entity_id: Option<u32>,
    #[serde(default = "default_skybox_path")]
    pub skybox_path: String,
    #[serde(default = "default_ambient_color")]
    pub ambient_color: Vec3,
    #[serde(default = "default_ambient_intensity")]
    pub ambient_intensity: f32,
    #[serde(default, skip_serializing_if = "crate::AmbientSettings::is_default")]
    pub ambient: crate::AmbientSettings,
    #[serde(default)]
    pub nav_settings: NavMeshSettings,
    #[serde(default)]
    pub fog: crate::FogSettings,
    #[serde(default, skip_serializing_if = "crate::SkySettings::is_default")]
    pub sky: crate::SkySettings,
    #[serde(default)]
    pub layers: LayerRegistry,
    #[serde(default)]
    pub collision_matrix: CollisionMatrix,
    #[serde(default)]
    pub materials: BTreeMap<String, MaterialAsset>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub meshes: crate::mesh_library::MeshLibrary,
    #[serde(default)]
    pub probes: crate::lighting::probe::ProbeVolume,
    #[serde(default)]
    pub reflection_probes: crate::lighting::reflection_probe::ReflectionProbeSet,
    #[serde(default, skip_serializing_if = "LightmapSet::is_empty")]
    pub lightmaps: LightmapSet,
    #[serde(default, skip_serializing_if = "LightingSettings::is_default")]
    pub lighting_settings: LightingSettings,
}
