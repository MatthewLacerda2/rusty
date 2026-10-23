//! src/scene/serialize.rs — World <-> SceneData
//!
//! Converts between the runtime hecs-backed `Scene` and the serde `SceneData`
//! document:
//!   to_scene_data(&Scene)             -> SceneData   (read component VALUES out)
//!   apply_scene_data(&mut Scene, ..)  -> ()          (rebuild the World)
//!
//! `SceneData` is the on-disk format: entity component values + scene settings,
//! never baked GPU buffers. On load we rehydrate the non-serialized data —
//! primitive meshes from `primitive_type`, and the collider world AABBs (which
//! `#[serde(skip)]` keeps out of the file) — so the file stays human-readable,
//! diffable and small, and never holds a box gone stale.

use std::collections::BTreeMap;

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::components::{Entity, MaterialAsset};
use crate::scene::authoring::material::fold_pending;
use crate::scene::collision_matrix::CollisionMatrix;
use crate::scene::layers::LayerRegistry;
use crate::scene::lighting::lightmap::LightmapSet;
use crate::scene::lighting::LightingSettings;
use crate::scene::nav_settings::NavMeshSettings;
use crate::scene::Scene;

pub use super::rehydrate::{asset_mesh_component, asset_ref_file, rehydrate_entity_mesh};

fn default_skybox_path() -> String {
    String::new()
}
// The ambient serde defaults mirror `Scene::default` via the engine's single source
// of truth (`scene::DEFAULT_AMBIENT_*`), so a scene file that omits these fields
// loads with the SAME brightened hemisphere ambient a freshly created scene gets.
fn default_ambient_color() -> Vec3 {
    crate::scene::DEFAULT_AMBIENT_COLOR
}
fn default_ambient_intensity() -> f32 {
    crate::scene::DEFAULT_AMBIENT_INTENSITY
}

/// The on-disk scene document: entity component VALUES plus scene-level settings.
///
/// This is deliberately separate from the runtime hecs `World`: hecs does not
/// serialize a World for free, and we never want GPU buffers in the file. Its
/// `#[serde(default)]`s let a sparse, hand-written scene omit a block; no older
/// format is read (CLAUDE.md, *No backward compatibility*).
#[derive(Serialize, Deserialize)]
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
    /// Per-scene navmesh bake settings (#276). A scene that omits the block loads
    /// with the historical bake constants — and the per-field serde defaults inside `NavMeshSettings` mean a partial block (only
    /// some knobs authored) fills the rest in too.
    #[serde(default)]
    pub nav_settings: NavMeshSettings,
    /// Distance + height fog (#437); off when absent.
    #[serde(default)]
    pub fog: crate::scene::FogSettings,
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
    /// Light-probe POSITIONS + grid layout (#240). The probes' baked SH is
    /// `#[serde(skip)]` on `Probe`, so this document carries only positions/layout
    /// (references + values, no GPU/heavy data); the SH lives in the
    /// `<scene>.lighting.json` sidecar. None when absent.
    #[serde(default)]
    pub probes: crate::scene::lighting::probe::ProbeVolume,
    /// Reflection-probe positions, parallax boxes, and cubemap PATHS (#244) — KTX2
    /// files, never inlined, like `skybox_path`. None when absent.
    #[serde(default)]
    pub reflection_probes: crate::scene::lighting::reflection_probe::ReflectionProbeSet,
    /// Which lightmap file each static mesh wears (#438). Paths only; the RGBM PNGs
    /// sit beside the scene. Empty — and left out of the file — by default.
    #[serde(default, skip_serializing_if = "LightmapSet::is_empty")]
    pub lightmaps: LightmapSet,
    /// The lightmap bake's knobs (#832). Left out while every one is at its default,
    /// so an untouched scene reads and writes without the block.
    #[serde(default, skip_serializing_if = "LightingSettings::is_default")]
    pub lighting_settings: LightingSettings,
}

/// Read the live World's component values out into a serializable document.
pub fn to_scene_data(scene: &Scene) -> SceneData {
    SceneData {
        entities: {
            // Bones are rebuilt from the model on load, never saved (#453).
            let mut entities = scene.world.collect_entities();
            crate::scene::skeleton::strip_bones(&mut entities);
            entities
        },
        next_entity_id: crate::scene::skeleton::saved_next_id(scene),
        selected_entity_id: scene.selected_entity_id,
        skybox_path: scene.skybox_path.clone(),
        ambient_color: scene.ambient_color,
        ambient_intensity: scene.ambient_intensity,
        nav_settings: scene.nav_settings.clone(),
        fog: scene.fog,
        layers: scene.layers.clone(),
        collision_matrix: scene.collision_matrix.clone(),
        materials: scene.materials.clone(),
        probes: scene.probes.clone(),
        reflection_probes: scene.reflection_probes.clone(),
        lightmaps: scene.lightmaps.clone(),
        lighting_settings: scene.lighting_settings,
    }
}

/// Rebuild every mesh in the document from its reference (see [`rehydrate_one`]).
fn rehydrate_meshes(data: &mut SceneData) {
    for entity in &mut data.entities {
        rehydrate_entity_mesh(entity);
    }
}

/// Replace the scene's World (single active scene) with the document's contents,
/// rehydrating meshes (stamping `meshes_read_at`, #1242) and recomputing collider
/// AABBs. Preserves entity order/ids.
pub fn apply_scene_data(scene: &mut Scene, mut data: SceneData) {
    rehydrate_meshes(&mut data);
    scene.meshes_read_at = crate::core::fs::written::written_since(u64::MAX).0;

    // Start from the document's library, then fold in any material an entity still
    // has staged (an Add-menu default restored from a snapshot before the inspector
    // folded it in); an authored asset already under the key is kept (#997). Done
    // before spawning so each entity's reference resolves.
    scene.materials = data.materials;
    for entity in &mut data.entities {
        if let Some(pending) = entity.pending_material.take() {
            if let Some(reference) = &entity.material {
                fold_pending(&mut scene.materials, &reference.material, pending);
            }
        }
    }

    let bone_parents = crate::scene::skeleton::take_bone_parents(&mut data.entities);
    let bone_refs = crate::scene::skeleton::take_bone_refs(&mut data.entities);
    scene.world.clear();
    // The overrides named the replaced World's entities; they are runtime-only (#670).
    scene.shader_overrides.clear_all();
    for entity in data.entities {
        scene.world.insert_entity(entity);
    }
    scene.world.bump_next_id(data.next_entity_id);
    scene.selected_entity_id = data.selected_entity_id;
    scene.skybox_path = data.skybox_path;
    scene.ambient_color = data.ambient_color;
    scene.ambient_intensity = data.ambient_intensity;
    scene.nav_settings = data.nav_settings;
    scene.fog = data.fog;
    data.layers.normalize();
    scene.layers = data.layers;
    scene.collision_matrix = data.collision_matrix;
    // Probe positions + grid come from the scene doc; their SH stays zero here and is
    // merged in from the `<scene>.lighting.json` sidecar by `load_from_file` (#240).
    scene.probes = data.probes;
    // Reflection probes (positions + boxes + cubemap paths) come straight from the
    // scene doc; the cubemaps themselves are loaded lazily by the renderer (#244).
    scene.reflection_probes = data.reflection_probes;
    scene.lightmaps = data.lightmaps;
    scene.lighting_settings = data.lighting_settings;

    // Rebuild every skeleton from its model, then hang the saved attachments back
    // on their bones by name (#453) — before prefab propagation, which rebuilds a
    // linked skinned entity from its source and would drop its bone overrides.
    scene.hang_on_skeletons(bone_parents, bone_refs);

    scene.update_all_colliders();

    // Propagate every linked prefab instance against its source on load (#216):
    // re-baseline from the current `.prefab`, then re-apply each instance's recorded
    // overrides on top. A missing/renamed source is skipped per instance (the
    // instance keeps its last-saved values), so this never aborts the load.
    crate::scene::prefab::link::reimport_all_linked_instances(scene);

    // Validate declared component dependencies on load (#359): a hand-edited or
    // drifted scene carrying a dependent component without its requirement (e.g. a
    // camera-less VisualCorrection) gets the requirement auto-added — the same rule
    // `add` uses — so the scene loads instead of silently violating it.
    for id in scene.world.ids().to_vec() {
        for (dependent, requirement) in
            crate::scene::authoring::dependency::enforce_requirements(&mut scene.world, id)
        {
            log::warn!(
                "[Scene] entity {id}: {dependent:?} requires {requirement:?}; auto-added a default \
                 {requirement:?} (RequireComponent)."
            );
        }
    }
}
