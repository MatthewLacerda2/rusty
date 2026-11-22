//! stored-format: the scene document (`SceneData`), World <-> serde

use crate::authoring::material::fold_pending;
use rusty_scene::lighting::lightmap_set::LightmapSet;
use rusty_scene::serialize::{RehydratedScene, SceneData};
use rusty_scene::Scene;

pub fn to_scene_data(scene: &Scene) -> SceneData {
    SceneData {
        entities: {
            let mut entities = scene.world.collect_documents();
            crate::prefab::live::write_overrides(&mut entities);
            rusty_scene::skeleton::strip_bones(&mut entities);
            entities
        },
        next_entity_id: rusty_scene::skeleton::saved_next_id(scene),
        selected_entity_id: scene.selected_entity_id,
        skybox_path: scene.skybox_path.clone(),
        ambient_color: scene.ambient_color,
        ambient_intensity: scene.ambient_intensity,
        ambient: scene.ambient,
        nav_settings: scene.nav_settings.clone(),
        fog: scene.fog,
        sky: scene.sky,
        layers: scene.layers.clone(),
        collision_matrix: scene.collision_matrix.clone(),
        materials: (*scene.materials).clone(),
        meshes: scene.meshes.clone(),
        probes: scene.probes.clone(),
        reflection_probes: scene.reflection_probes.clone(),
        lightmaps: saved_lightmaps(scene),
        lighting_settings: scene.lighting_settings,
    }
}

/// A bake landing after a delete can hold dead entities' entries; a file names only live ones.
fn saved_lightmaps(scene: &Scene) -> LightmapSet {
    let mut set = scene.lightmaps.saved().clone();
    set.retain(|id| scene.world.contains(id));
    set
}

pub fn apply_scene_data(scene: &mut Scene, data: SceneData) {
    apply_rehydrated(scene, RehydratedScene::new(data));
}

pub fn apply_rehydrated(scene: &mut Scene, document: RehydratedScene) {
    apply(scene, document, true);
}

pub fn apply_world_data(scene: &mut Scene, data: SceneData) {
    let document = RehydratedScene::adopting(data, &mut scene.world);
    apply(scene, document, false);
}

fn take_materials(scene: &mut Scene, data: &mut SceneData) {
    scene.materials = std::mem::take(&mut data.materials).into();
    for entity in &mut data.entities {
        if let Some(pending) = entity.pending_material.take() {
            if let Some(reference) = &entity.material {
                fold_pending(&mut scene.materials, reference.first(), pending);
            }
        }
    }
}

fn drop_old_world(scene: &mut Scene) {
    scene.world.clear();
    scene.clear_undo();
    scene.shader_overrides.clear_all();
    scene.ragdolls = Default::default();
    // Persistent decals too: a Play load took them out first (`load::swap_in_prepared`), and
    // Stop's restore puts its own back.
    scene.clear_decals(false);
    #[cfg(feature = "dev")]
    scene.drop_bake_request("another scene was loaded");
    // A Play load leaves the `Debug.Draw*` lines to age out; an edit-mode load drops them.
    if !scene.is_playing() {
        scene.debug_lines = Default::default();
    }
}

fn apply(scene: &mut Scene, document: RehydratedScene, from_disk: bool) {
    let (mut data, read_at, mut imports) = document.into_parts();
    scene.meshes_read_at = read_at;

    take_materials(scene, &mut data);
    rusty_scene::mesh_library::put_back(std::mem::take(&mut data.meshes), &mut scene.meshes);

    let bone_parents = rusty_scene::skeleton::take_bone_parents(&mut data.entities);
    let bone_refs = rusty_scene::skeleton::take_bone_refs(&mut data.entities);
    drop_old_world(scene);
    for entity in data.entities {
        scene.world.insert_entity(entity);
    }
    scene.world.bump_next_id(data.next_entity_id);
    scene.selected_entity_id = data.selected_entity_id;
    scene.skybox_path = data.skybox_path;
    scene.ambient_color = data.ambient_color;
    scene.ambient_intensity = data.ambient_intensity;
    scene.ambient = crate::authoring::environment::held_ambient(data.ambient);
    scene.nav_settings = data.nav_settings;
    scene.fog = data.fog;
    scene.sky = crate::authoring::environment::held_sky(data.sky);
    data.layers.normalize();
    scene.layers = data.layers;
    scene.collision_matrix = data.collision_matrix;
    scene.probes = data.probes;
    scene.reflection_probes = data.reflection_probes;
    scene.reflection_probes.hold();
    scene.inspected_reflection_probe = None;
    scene.lightmaps = data.lightmaps.into();
    scene.lighting_settings = data.lighting_settings;
    scene.lighting_settings.reflections.clamp();
    let lightmaps = &mut scene.lighting_settings.lightmaps;
    *lightmaps = lightmaps.sanitized();

    // Before prefab propagation, which rebuilds a linked skinned entity from its source and
    // would drop its bone overrides.
    scene.hang_on_skeletons(bone_parents, bone_refs);

    // Before the collider AABBs, which read the held shapes.
    if from_disk {
        crate::authoring::fields::hold_world(&mut scene.world);
        scene.meshes = rusty_scene::mesh_library::checked(std::mem::take(&mut scene.meshes));
    }

    scene.update_all_colliders();

    crate::prefab::link::propagate_linked_instances_in(scene, &mut imports);
    scene.sync_scene_meshes();

    for id in scene.world.ids().to_vec() {
        for (dependent, requirement) in
            crate::authoring::dependency::enforce_requirements(&mut scene.world, id)
        {
            log::warn!(
                "[Scene] entity {id}: {dependent:?} requires {requirement:?}; auto-added a default \
                 {requirement:?} (RequireComponent)."
            );
        }
    }
}

#[cfg(test)]
#[path = "serialize_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "geometry_tests.rs"]
mod geometry_tests;
