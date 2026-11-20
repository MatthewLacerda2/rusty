//! crates/scene/src/serialize.rs — the scene document, `SceneData`
//!
//! `SceneData` is the on-disk format: entity component values + scene settings,
//! never baked GPU buffers. On load the non-serialized data is rehydrated
//! (`rehydrate`): primitive meshes from `primitive_type`, and the collider world
//! AABBs (which `#[serde(skip)]` keeps out of the file), so the file stays
//! human-readable, diffable and small, and never holds a box gone stale.
//!
//! The conversion between a live `Scene` and this document (`to_scene_data`, the
//! `apply_*` functions) is the edit side's, `rusty_edit::serialize`: it runs the
//! authoring holds and prefab propagation.
//!
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
// The ambient serde defaults mirror `Scene::default` via the engine's single source
// of truth (`scene::DEFAULT_AMBIENT_*`), so a scene file that omits these fields
// loads with the SAME brightened hemisphere ambient a freshly created scene gets.
fn default_ambient_color() -> Vec3 {
    crate::DEFAULT_AMBIENT_COLOR
}
fn default_ambient_intensity() -> f32 {
    crate::DEFAULT_AMBIENT_INTENSITY
}

/// The on-disk scene document: entity component VALUES plus scene-level settings.
///
/// This is deliberately separate from the runtime hecs `World`: hecs does not
/// serialize a World for free, and we never want GPU buffers in the file. Its
/// `#[serde(default)]`s let a sparse, hand-written scene omit a block; no older
/// format is read (CLAUDE.md, *Backward compatibility is not a concern*).
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
    /// The ambient source and the gradient's colours (#1647); `Color` when absent.
    #[serde(default, skip_serializing_if = "crate::AmbientSettings::is_default")]
    pub ambient: crate::AmbientSettings,
    /// Per-scene navmesh bake settings (#276); a knob left out takes its default.
    #[serde(default)]
    pub nav_settings: NavMeshSettings,
    /// Distance + height fog (#437); off when absent.
    #[serde(default)]
    pub fog: crate::FogSettings,
    /// The procedural sky's look (#1620). Left out while every knob is at its
    /// default.
    #[serde(default, skip_serializing_if = "crate::SkySettings::is_default")]
    pub sky: crate::SkySettings,
    /// Project layer names; the stock registry ("Default" + 31 unnamed slots) when
    /// absent.
    #[serde(default)]
    pub layers: LayerRegistry,
    /// Layer collision matrix; the all-pairs-collide default when absent.
    #[serde(default)]
    pub collision_matrix: CollisionMatrix,
    /// The material library: reusable materials keyed by name (#201). Entities
    /// reference one by name. Empty when absent.
    #[serde(default)]
    pub materials: BTreeMap<String, MaterialAsset>,
    /// The meshes made from data (#2059), by name. Left out when there are none.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub meshes: crate::mesh_library::MeshLibrary,
    /// Light-probe POSITIONS + grid layout (#240). The probes' baked SH is
    /// `#[serde(skip)]` on `Probe`, so this document carries only positions/layout
    /// (references + values, no GPU/heavy data); the SH lives in the
    /// `<scene>.lighting.json` sidecar. None when absent.
    #[serde(default)]
    pub probes: crate::lighting::probe::ProbeVolume,
    /// Reflection-probe positions, parallax boxes, and cubemap PATHS (#244) — KTX2
    /// files, never inlined, like `skybox_path`. None when absent.
    #[serde(default)]
    pub reflection_probes: crate::lighting::reflection_probe::ReflectionProbeSet,
    /// Which lightmap page each static mesh wears (#438). Paths only; the KTX2 pages
    /// (#1694) sit beside the scene. Empty — and left out of the file — by default.
    #[serde(default, skip_serializing_if = "LightmapSet::is_empty")]
    pub lightmaps: LightmapSet,
    /// The lightmap bake's knobs (#832). Left out while every one is at its default,
    /// so an untouched scene reads and writes without the block.
    #[serde(default, skip_serializing_if = "LightingSettings::is_default")]
    pub lighting_settings: LightingSettings,
}
