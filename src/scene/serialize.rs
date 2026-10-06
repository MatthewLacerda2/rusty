//! src/scene/serialize.rs — World <-> SceneData
//!
//! Converts between the runtime hecs-backed `Scene` and the serde `SceneData`
//! document:
//!   to_scene_data(&Scene)             -> SceneData   (read component VALUES out)
//!   apply_scene_data(&mut Scene, ..)  -> ()          (rebuild the World)
//!
//! `SceneData` is the on-disk format: entity component values + scene settings,
//! never baked GPU buffers. On load we rehydrate the non-serialized data —
//! primitive meshes from `primitive_type`, and the collider world AABBs — so the
//! file stays human-readable/diffable and small.

use std::collections::BTreeMap;

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::components::{Entity, MaterialAsset};
use crate::scene::collision_matrix::CollisionMatrix;
use crate::scene::layers::LayerRegistry;
use crate::scene::lighting::lightmap::LightmapSet;
use crate::scene::lighting::LightingSettings;
use crate::scene::nav_settings::NavMeshSettings;
use crate::scene::Scene;

pub use super::rehydrate::{asset_mesh_component, rehydrate_entity_mesh};

fn default_skybox_path() -> String {
    String::new()
}
// The ambient serde defaults mirror `Scene::default` via the engine's single source
// of truth (`scene::DEFAULT_AMBIENT_*`), so a pre-#256 scene that omits these fields
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
/// serialize a World for free, and we never want GPU buffers in the file. The
/// JSON layout matches the legacy `Scene` format so existing `.scene` files
/// round-trip unchanged.
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
    /// Per-scene navmesh bake settings (#276). `#[serde(default)]` so a pre-#276
    /// scene that omits the block loads with the historical bake constants — and the
    /// per-field serde defaults inside `NavMeshSettings` mean a partial block (only
    /// some knobs authored) fills the rest in too.
    #[serde(default)]
    pub nav_settings: NavMeshSettings,
    /// Distance + height fog (#437); `#[serde(default)]`: older scenes load it off.
    #[serde(default)]
    pub fog: crate::scene::FogSettings,
    /// Project layer names. `#[serde(default)]` so pre-#90 scenes load with the
    /// stock registry ("Default" + 31 unnamed slots).
    #[serde(default)]
    pub layers: LayerRegistry,
    /// Layer collision matrix. `#[serde(default)]` so pre-#91 scenes load with the
    /// all-pairs-collide default (preserving their behaviour).
    #[serde(default)]
    pub collision_matrix: CollisionMatrix,
    /// The material library: reusable materials keyed by name (#201). Entities
    /// reference one by name. `#[serde(default)]` so pre-#201 scenes (which stored
    /// the material inline per entity) load with an empty library; their inline
    /// materials are migrated in [`apply_scene_data`].
    #[serde(default)]
    pub materials: BTreeMap<String, MaterialAsset>,
    /// Light-probe POSITIONS + grid layout (#240). The probes' baked SH is
    /// `#[serde(skip)]` on `Probe`, so this document carries only positions/layout
    /// (references + values, no GPU/heavy data); the SH lives in the
    /// `<scene>.lighting.json` sidecar. `#[serde(default)]` for pre-#240 scenes.
    #[serde(default)]
    pub probes: crate::scene::lighting::probe::ProbeVolume,
    /// Reflection-probe positions, parallax boxes, and cubemap PATHS (#244) — KTX2
    /// files, never inlined, like `skybox_path`. `#[serde(default)]` for pre-#244 scenes.
    #[serde(default)]
    pub reflection_probes: crate::scene::lighting::reflection_probe::ReflectionProbeSet,
    /// Which lightmap file each static mesh wears (#438). Paths only; the RGBM PNGs
    /// sit beside the scene. Default-empty so pre-#438 scenes load with none.
    #[serde(default, skip_serializing_if = "LightmapSet::is_empty")]
    pub lightmaps: LightmapSet,
    /// The lightmap bake's knobs (#832). Left out while every one is at its default,
    /// so older scenes and untouched ones read and write unchanged.
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
/// rehydrating meshes and recomputing collider AABBs. Preserves entity order/ids.
pub fn apply_scene_data(scene: &mut Scene, mut data: SceneData) {
    rehydrate_meshes(&mut data);

    // Start from the document's library (new-format scenes), then fold in any legacy
    // inline materials migration carriers lifted out of pre-#201 entities by
    // `From<EntityRepr>`. Done before spawning so each entity's reference resolves.
    scene.materials = data.materials;
    for entity in &mut data.entities {
        if let Some(pending) = entity.pending_material.take() {
            if let Some(reference) = &entity.material {
                scene.materials.insert(reference.material.clone(), pending);
            }
        }
    }

    let bone_parents = crate::scene::skeleton::take_bone_parents(&mut data.entities);
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
    scene.sync_skeletons();
    scene.attach_to_bones(bone_parents);

    scene.update_all_colliders();

    // Propagate every linked prefab instance against its source on load (#216):
    // re-baseline from the current `.prefab`, then re-apply each instance's recorded
    // overrides on top. A missing/renamed source is skipped per instance (the
    // instance keeps its last-saved values), so this never aborts the load.
    crate::scene::prefab::link::reimport_all_linked_instances(scene);

    // Validate declared component dependencies on load (#359): a hand-edited or
    // drifted scene carrying a dependent component without its requirement (e.g. a
    // camera-less VisualCorrection) gets the requirement auto-added — the same rule
    // `add` uses — so old scenes keep loading instead of silently violating it.
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
