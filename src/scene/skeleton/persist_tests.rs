//! Saving a skeleton (#453): bones are rebuilt from the model, never saved; a
//! designer's bone overrides and the entities hung on a bone survive by name.

use glam::{Quat, Vec3};

use super::fixture::hero;
use crate::scene::serialize::{apply_scene_data, to_scene_data, SceneData};
use crate::scene::{Scene, SceneSnapshot};

/// Save `scene` to JSON (the on-disk path) and load it into a fresh scene.
fn reload(scene: &Scene) -> Scene {
    let json = serde_json::to_string(&to_scene_data(scene)).unwrap();
    reload_json(&json)
}

fn reload_json(json: &str) -> Scene {
    let data: SceneData = serde_json::from_str(json).unwrap();
    let mut out = Scene::new();
    apply_scene_data(&mut out, data);
    out
}

/// A hero whose `Joint1` the designer turned, holding a `Gun` under `Joint1`.
fn armed_hero(tag: &str) -> (Scene, u32) {
    let mut scene = Scene::new();
    let hero = hero(&mut scene, tag);
    let joint1 = scene.find_bone(hero, "Joint1").unwrap();
    scene.world.transform_mut(joint1).unwrap().rotation = Quat::from_rotation_z(0.5);
    let gun = scene.add_entity("Gun".to_string());
    scene.world.transform_mut(gun).unwrap().position = Vec3::new(0.0, 0.25, 0.0);
    scene.set_parent(gun, Some(joint1)).unwrap();
    (scene, hero)
}

#[test]
fn a_saved_scene_holds_no_bones_only_overrides_and_bone_attachments() {
    let (scene, hero) = armed_hero("doc");
    let data = to_scene_data(&scene);
    let names: Vec<&str> = data.entities.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(
        names,
        ["Hero", "Gun"],
        "bones are rebuilt from the model, not saved"
    );
    let owner = &data.entities[0];
    let overrides = &owner.mesh.as_ref().unwrap().skeleton.overrides;
    assert_eq!(
        overrides.keys().collect::<Vec<_>>(),
        ["Joint1"],
        "an untouched bone is no override"
    );
    let gun = &data.entities[1];
    assert_eq!(gun.parent_id, Some(hero));
    assert_eq!(gun.parent_bone.as_deref(), Some("Joint1"));
    assert_eq!(owner.children, vec![gun.id]);
}

#[test]
fn save_and_load_restores_overrides_and_rehangs_attachments_by_name() {
    let (scene, _) = armed_hero("roundtrip");
    let loaded = reload(&scene);
    let hero = loaded.find_entity_by_name("Hero").unwrap();
    let joint1 = loaded
        .find_bone(hero, "Joint1")
        .expect("the skeleton is rebuilt");
    let turned = loaded.world.transform(joint1).unwrap().rotation;
    assert!(turned.abs_diff_eq(Quat::from_rotation_z(0.5), 1e-6));
    let gun = loaded.find_entity_by_name("Gun").unwrap();
    assert_eq!(loaded.world.parent_id(gun), Some(joint1));
    let held = loaded.world.transform(gun).unwrap().position;
    assert!(held.abs_diff_eq(Vec3::new(0.0, 0.25, 0.0), 1e-6));
    // Saving the loaded scene again yields the same document.
    let again = serde_json::to_string(&to_scene_data(&loaded)).unwrap();
    assert_eq!(
        again,
        serde_json::to_string(&to_scene_data(&reload_json(&again))).unwrap()
    );
}

#[test]
fn an_override_or_attachment_whose_bone_vanished_is_dropped_not_fatal() {
    let (scene, _) = armed_hero("vanished");
    // The model was re-exported with `Joint1` renamed: the saved names no longer match.
    let json = serde_json::to_string(&to_scene_data(&scene))
        .unwrap()
        .replace("Joint1", "OldBone");
    let loaded = reload_json(&json);
    let hero = loaded.find_entity_by_name("Hero").unwrap();
    let joint1 = loaded.find_bone(hero, "Joint1").unwrap();
    // The stale override is dropped (with a warning): the bone stays at rest.
    let rest = loaded.world.transform(joint1).unwrap().rotation;
    assert!(rest.abs_diff_eq(Quat::IDENTITY, 1e-6));
    assert!(loaded
        .world
        .mesh(hero)
        .unwrap()
        .skeleton
        .overrides
        .is_empty());
    // The gun stays on the character rather than being lost.
    let gun = loaded.find_entity_by_name("Gun").unwrap();
    assert_eq!(loaded.world.parent_id(gun), Some(hero));
}

#[test]
fn stopping_play_restores_the_skeleton_and_attachments() {
    let (mut scene, hero) = armed_hero("snapshot");
    let snapshot = SceneSnapshot::capture(&scene);
    let joint1 = scene.find_bone(hero, "Joint1").unwrap();
    scene.world.transform_mut(joint1).unwrap().position = Vec3::splat(9.0); // played
    snapshot.restore(&mut scene);
    let joint1 = scene.find_bone(hero, "Joint1").unwrap();
    assert!(scene
        .world
        .transform(joint1)
        .unwrap()
        .position
        .abs_diff_eq(Vec3::Y, 1e-6));
    let gun = scene.find_entity_by_name("Gun").unwrap();
    assert_eq!(scene.world.parent_id(gun), Some(joint1));
}

#[test]
fn a_prefab_of_a_character_keeps_its_bone_attachment() {
    let (mut scene, hero) = armed_hero("prefab");
    let prefab = crate::scene::extract_prefab(&scene, hero).unwrap();
    assert_eq!(prefab.entities.len(), 2, "the prefab holds no bones");
    let copy = crate::scene::instantiate_prefab(&mut scene, &prefab, None);
    let joint1 = scene
        .find_bone(copy, "Joint1")
        .expect("the copy has its own skeleton");
    let turned = scene.world.transform(joint1).unwrap().rotation;
    assert!(turned.abs_diff_eq(Quat::from_rotation_z(0.5), 1e-6));
    let held = scene.world.children(joint1);
    assert_eq!(held.len(), 1);
    assert_eq!(*scene.world.name(held[0]).unwrap(), "Gun");
}

/// A linked prefab instance is rebuilt from its source on every load; the bones a
/// designer turned on the instance, and the gun its source hangs on a bone, must
/// survive that propagation.
#[test]
fn a_linked_prefab_instance_keeps_its_bone_overrides_through_propagation() {
    let (source, hero) = armed_hero("linked");
    let path = crate::test_temp::dir().join("rusty_453_linked.prefab");
    let path = path.to_string_lossy().replace('\\', "/");
    crate::scene::save_prefab(&source, hero, &path).unwrap();

    let mut scene = Scene::new();
    let instance = crate::scene::load_and_instantiate_linked(&mut scene, &path, None).unwrap();
    let joint0 = scene.find_bone(instance, "Joint0").unwrap();
    scene.world.transform_mut(joint0).unwrap().scale = Vec3::splat(2.0);

    let loaded = reload(&scene);
    let hero = loaded.find_entity_by_name("Hero").unwrap();
    let joint0 = loaded.find_bone(hero, "Joint0").unwrap();
    let joint1 = loaded.find_bone(hero, "Joint1").unwrap();
    let scale = loaded.world.transform(joint0).unwrap().scale;
    assert!(
        scale.abs_diff_eq(Vec3::splat(2.0), 1e-6),
        "the instance's override"
    );
    let turned = loaded.world.transform(joint1).unwrap().rotation;
    assert!(
        turned.abs_diff_eq(Quat::from_rotation_z(0.5), 1e-6),
        "the source's override"
    );
    let gun = loaded.find_entity_by_name("Gun").unwrap();
    assert_eq!(loaded.world.parent_id(gun), Some(joint1));
    assert!(
        loaded.world.prefab_link(gun).is_some(),
        "the gun is still part of the instance"
    );
}
