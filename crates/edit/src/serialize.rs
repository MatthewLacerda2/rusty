//! crates/edit/src/serialize.rs — World <-> SceneData
//!
//! Converts between the runtime hecs-backed `Scene` and the serde `SceneData`
//! document (`rusty_scene::serialize`, the base's):
//!   to_scene_data(&Scene)             -> SceneData   (read component VALUES out)
//!   apply_scene_data(&mut Scene, ..)  -> ()          (rebuild the World, holding loaded values)
//!   apply_world_data(&mut Scene, ..)  -> ()          (rebuild from a Play snapshot, not held again)
//!
//! A save writes each linked instance's overrides and strips the bones; a load
//! holds every value through its authoring op and propagates prefab instances, so
//! this half names `authoring` and `prefab` and sits on the edit side.
//!
//! stored-format: the scene document (`SceneData`), World <-> serde

use crate::authoring::material::fold_pending;
use rusty_scene::lighting::lightmap_set::LightmapSet;
use rusty_scene::serialize::{RehydratedScene, SceneData};
use rusty_scene::Scene;

/// Read the live World's component values out into a serializable document.
pub fn to_scene_data(scene: &Scene) -> SceneData {
    SceneData {
        entities: {
            // Without geometry: every reader drops it (a save skips the fields, a
            // restore rebuilds or adopts it). Bones are rebuilt from the model on
            // load, never saved (#453).
            let mut entities = scene.world.collect_documents();
            // A linked instance's edits are overrides the moment they are made (#1527).
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

/// The lightmaps a save writes ([`LightmapSet::saved`]), less the entries of
/// entities no longer live: a bake that finished after one was deleted keeps its
/// entry in the scene, where an Undo of the delete finds it (#1970), but a file
/// never names an entity it does not hold.
fn saved_lightmaps(scene: &Scene) -> LightmapSet {
    let mut set = scene.lightmaps.saved().clone();
    set.retain(|id| scene.world.contains(id));
    set
}

/// Replace the scene's World (single active scene) with the document's contents,
/// rehydrating meshes (stamping `meshes_read_at`, #1242), holding every loaded
/// number through its authoring op (#1418) and recomputing collider AABBs.
/// Preserves entity order/ids.
pub fn apply_scene_data(scene: &mut Scene, data: SceneData) {
    apply_rehydrated(scene, RehydratedScene::new(data));
}

/// [`apply_scene_data`] for a document already rehydrated.
pub fn apply_rehydrated(scene: &mut Scene, document: RehydratedScene) {
    apply(scene, document, true);
}

/// [`apply_scene_data`] for a document the World itself wrote (a Play snapshot):
/// its values were held when they came in, so they are not held again, and each
/// mesh Play left as it was (the same source and revision, current against the
/// import cache) takes the live World's geometry before that World is cleared,
/// instead of being imported again (`RehydratedScene::adopting`). Linked prefab
/// instances re-read their source and re-apply the overrides the snapshot took, so
/// the World comes back as captured, with any change made to a source during Play
/// (#1527); an instance part whose geometry is still current keeps it there too.
pub fn apply_world_data(scene: &mut Scene, data: SceneData) {
    let document = RehydratedScene::adopting(data, &mut scene.world);
    apply(scene, document, false);
}

/// Start from the document's library, then fold in any material an entity still
/// has staged (an Add-menu default restored from a snapshot before the inspector
/// folded it in); an authored asset already under the key is kept (#997). Done
/// before spawning so each entity's reference resolves.
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

/// Clear the World and what named or was drawn over its entities, before a
/// document's entities come in.
fn drop_old_world(scene: &mut Scene) {
    scene.world.clear();
    // The Undo history named the replaced World's entities (#1631).
    scene.clear_undo();
    // The overrides named the replaced World's entities; they are runtime-only (#670).
    scene.shader_overrides.clear_all();
    // The ragdolls' runtime state named them too (#1895).
    scene.ragdolls = Default::default();
    // So did the decals stamped on it: a load drops them, as Unity's scene load
    // destroys everything of the old scene (#1649). A Play load took the
    // persistent ones out first (`swap_in`), and Stop's snapshot puts its own back.
    scene.clear_decals(false);
    // A pending `Lighting.GenerateAsync()` asked about the replaced scene (#1999).
    #[cfg(feature = "dev")]
    scene.drop_bake_request("another scene was loaded");
    // An edit-mode load drops the `Debug.Draw*` lines drawn over the old World
    // (#1882), as Stop does; a Play load leaves them to age out, as it leaves
    // Play's other runtime state (`scene::state`).
    if !scene.is_playing() {
        scene.debug_lines = Default::default();
    }
}

/// `from_disk`: the document came from a file, so its values are held; a World
/// snapshot's already were.
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
    // Held to its ranges once, here, as a setter would hold it.
    scene.sky = crate::authoring::environment::held_sky(data.sky);
    data.layers.normalize();
    scene.layers = data.layers;
    scene.collision_matrix = data.collision_matrix;
    // Probe positions + grid; their SH is merged from the sidecar by `load_from_file` (#240).
    scene.probes = data.probes;
    // Reflection probes (#244; the renderer loads their cubemaps lazily), held to
    // their ranges once, here (#1584), so every reader sees the same values.
    scene.reflection_probes = data.reflection_probes;
    scene.reflection_probes.hold();
    scene.lightmaps = data.lightmaps.into();
    scene.lighting_settings = data.lighting_settings;
    scene.lighting_settings.reflections.clamp();
    // The bake's knobs too (#1704): a hand-edited Max Lightmap Size of 5000 reads
    // back as the 4096 it bakes at.
    let lightmaps = &mut scene.lighting_settings.lightmaps;
    *lightmaps = lightmaps.sanitized();

    // Rebuild every skeleton from its model, then hang the saved attachments back
    // on their bones by name (#453) — before prefab propagation, which rebuilds a
    // linked skinned entity from its source and would drop its bone overrides.
    scene.hang_on_skeletons(bone_parents, bone_refs);

    // Every loaded number through its authoring op, once (#1418): the World never
    // holds a value its own setters would refuse. Before the collider AABBs, which
    // read the held shapes; prefab propagation holds what it rebuilds.
    if from_disk {
        crate::authoring::fields::hold_world(&mut scene.world);
        scene.meshes = rusty_scene::mesh_library::checked(std::mem::take(&mut scene.meshes));
    }

    scene.update_all_colliders();

    // Propagate every linked prefab instance against its source (#216): re-baseline
    // from the current `.prefab`, then re-apply each instance's saved overrides on
    // top. A Play snapshot is propagated too (#1527): its bones came back with fresh
    // ids, and a source applied to during Play reaches the edit-mode instances. A
    // missing/renamed source is skipped per instance, so this never aborts the load.
    crate::prefab::link::propagate_linked_instances_in(scene, &mut imports);
    // The entities drawing a library mesh (#2059) take its geometry.
    scene.sync_scene_meshes();

    // Validate declared component dependencies on load (#359): a dependent without
    // its requirement (a camera-less PostProcessing) gets it added, as `add` does.
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
