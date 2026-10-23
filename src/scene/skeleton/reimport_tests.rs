//! A re-written skinned model rebuilds the rig and bones of the entities built from
//! it (#1252), keeping the bone overrides and what hangs from a bone.

use std::collections::BTreeSet;

use glam::Vec3;

use super::fixture::{animated_model, armature_owner};
use super::HITBOX_NAME;
use crate::asset::fixtures_anim::ANIMATED_GLTF;
use crate::components::ColliderComponent;
use crate::scene::asset_instance::instantiate_asset;
use crate::scene::model_writes::ModelWrites;
use crate::scene::serialize::{apply_scene_data, to_scene_data};
use crate::scene::Scene;

/// The rewrite that drops `Joint1` from the fixture's skin.
const DROP_JOINT1: (&str, &str) = (
    r#""joints": [ 1, 2 ], "inverseBindMatrices": 3"#,
    r#""joints": [ 1 ]"#,
);

/// Instantiate the animated fixture written under `tag`, and return the entity and
/// the model file's path.
fn hero(scene: &mut Scene, tag: &str) -> (u32, String) {
    let reference = animated_model(tag);
    let id = instantiate_asset(scene, &reference, Some("Hero"), Vec3::ZERO).unwrap();
    let path = reference.trim_end_matches("::Animated").to_string();
    (id, path)
}

/// Re-write the model at `path` as the engine does (so the write is noted), with
/// `from` replaced by `to` in the fixture's glTF.
fn rewrite(path: &str, from: &str, to: &str) {
    assert!(ANIMATED_GLTF.contains(from), "the fixture has {from:?}");
    let text = ANIMATED_GLTF.replace(from, to);
    crate::core::fs::written::write(std::path::Path::new(path), text.as_bytes()).unwrap();
}

/// Hang a `Gun` one unit forward of `owner`'s bone `bone`.
fn hang_gun(scene: &mut Scene, owner: u32, bone: &str) -> u32 {
    let gun = scene.add_entity("Gun".to_string());
    scene.world.transform_mut(gun).unwrap().position = Vec3::Z;
    let bone = scene.find_bone(owner, bone).unwrap();
    scene.set_parent(gun, Some(bone)).unwrap();
    gun
}

fn parent_name(scene: &Scene, id: u32) -> String {
    let parent = scene.world.parent_id(id).unwrap();
    scene.world.name(parent).unwrap().clone()
}

/// `old` are all gone, no live entity lists a dead child, and a saved scene names
/// none either.
fn assert_dead_and_unlisted(scene: &Scene, old: &[u32]) {
    for &id in old {
        assert!(!scene.world.contains(id), "entity {id} was despawned");
    }
    for id in scene.entity_ids() {
        for child in scene.world.children(id) {
            assert!(scene.world.contains(child), "{id} lists dead child {child}");
        }
    }
    let data = to_scene_data(scene);
    let saved: BTreeSet<u32> = data.entities.iter().map(|e| e.id).collect();
    for entity in &data.entities {
        assert!(entity.children.iter().all(|c| saved.contains(c)), "saved");
    }
}

#[test]
fn a_changed_bind_pose_rebuilds_the_rig_and_keeps_what_was_authored() {
    let mut scene = Scene::new();
    let (hero, path) = hero(&mut scene, "reimport_bind");
    let root = scene.find_bone(hero, "Joint0").unwrap();
    scene.world.transform_mut(root).unwrap().position = Vec3::new(0.5, 0.0, 0.0);
    let gun = hang_gun(&mut scene, hero, "Joint1");
    let old_palette = scene.world.mesh(hero).unwrap().bind_palette.clone();
    let old_bones = scene.world.mesh(hero).unwrap().skeleton.bones.clone();
    let mut writes = ModelWrites::default();

    rewrite(
        &path,
        r#""translation": [0.0, 1.0, 0.0]"#,
        r#""translation": [0.0, 2.0, 0.0]"#,
    );
    assert_eq!(writes.follow(&mut scene), 1);

    let mesh = scene.world.mesh(hero).unwrap();
    let skin = mesh.skin.clone().unwrap();
    assert_eq!(skin.local_bind[1].translation, 2.0 * Vec3::Y, "the new rig");
    assert_ne!(mesh.bind_palette, old_palette, "and its bind palette");
    assert!(mesh.is_dirty.get(), "re-uploaded");
    let bones = mesh.skeleton.bones.clone();
    drop(mesh);
    assert_eq!(bones.len(), 2);
    assert_eq!(scene.bone_ids().len(), 2, "the old bones are gone");
    let tip = scene.world.transform(bones[1]).unwrap().position;
    assert!(
        tip.abs_diff_eq(2.0 * Vec3::Y, 1e-6),
        "a bone rests where the new rig says"
    );
    let root = scene.world.transform(bones[0]).unwrap().position;
    assert_eq!(root, Vec3::new(0.5, 0.0, 0.0), "an override is kept");
    assert_eq!(
        scene.world.parent_id(gun),
        Some(bones[1]),
        "the gun hangs on"
    );
    assert_eq!(scene.world.transform(gun).unwrap().position, Vec3::Z);
    assert_dead_and_unlisted(&scene, &old_bones);
}

#[test]
fn a_removed_bone_hands_what_hung_from_it_to_its_parent_bone() {
    let mut scene = Scene::new();
    let (hero, path) = hero(&mut scene, "reimport_removed");
    let gun = hang_gun(&mut scene, hero, "Joint1");
    let hitbox = hang_gun(&mut scene, hero, "Joint1");
    scene.world.set_name(hitbox, HITBOX_NAME.to_string());
    let collider = ColliderComponent::from_mesh_bounds(true, -Vec3::ONE, Vec3::ONE);
    scene.world.set_collider(hitbox, Some(collider));
    let joint1 = scene.find_bone(hero, "Joint1").unwrap();
    scene.selected_entity_id = Some(joint1);
    let old_bones = scene.world.mesh(hero).unwrap().skeleton.bones.clone();
    let mut writes = ModelWrites::default();

    rewrite(&path, DROP_JOINT1.0, DROP_JOINT1.1);
    assert_eq!(writes.follow(&mut scene), 1);

    let bones = scene.world.mesh(hero).unwrap().skeleton.bones.clone();
    assert_eq!(bones.len(), 1, "the rig lost Joint1");
    assert_eq!(scene.world.parent_id(gun), Some(bones[0]));
    assert_eq!(parent_name(&scene, gun), "Joint0");
    assert!(!scene.world.contains(hitbox), "a hitbox goes with its bone");
    assert_eq!(scene.selected_entity_id, None, "the selected bone is gone");
    assert_dead_and_unlisted(&scene, &old_bones);
}

#[test]
fn a_reload_after_the_bone_went_hangs_the_same_way_a_live_reimport_does() {
    let mut scene = Scene::new();
    let (hero, path) = hero(&mut scene, "reimport_reload");
    hang_gun(&mut scene, hero, "Joint1");
    let saved = to_scene_data(&scene);

    rewrite(&path, DROP_JOINT1.0, DROP_JOINT1.1);
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, saved);

    let gun = loaded.find_entity_by_name("Gun").unwrap();
    assert_eq!(parent_name(&loaded, gun), "Joint0", "the nearest ancestor");
}

#[test]
fn a_model_rewritten_as_static_drops_its_bones_and_what_was_authored_on_them() {
    let mut scene = Scene::new();
    let (hero, path) = hero(&mut scene, "reimport_static");
    let joint1 = scene.find_bone(hero, "Joint1").unwrap();
    let body = crate::scene::authoring::default_rigidbody();
    scene.world.set_rigidbody(joint1, Some(body));
    scene.world.transform_mut(joint1).unwrap().position = Vec3::X;
    let gun = hang_gun(&mut scene, hero, "Joint1");
    let old_bones = scene.world.mesh(hero).unwrap().skeleton.bones.clone();
    let mut writes = ModelWrites::default();

    rewrite(&path, r#""mesh": 0, "skin": 0"#, r#""mesh": 0"#);
    assert_eq!(writes.follow(&mut scene), 1);

    let mesh = scene.world.mesh(hero).unwrap();
    assert!(mesh.skin.is_none(), "static now");
    assert!(mesh.skeleton.overrides.is_empty() && mesh.skeleton.bodies.is_empty());
    drop(mesh);
    assert_eq!(
        scene.world.parent_id(gun),
        Some(hero),
        "kept, on the entity"
    );
    assert_dead_and_unlisted(&scene, &old_bones);
}

#[test]
fn an_attachment_named_like_the_root_joint_is_not_taken_for_it() {
    let mut scene = Scene::new();
    let (hero, path) = hero(&mut scene, "reimport_namesake");
    let decoy = hang_gun(&mut scene, hero, "Joint1");
    scene.world.set_name(decoy, "Joint0".to_string());
    let mut writes = ModelWrites::default();

    rewrite(
        &path,
        r#""translation": [0.0, 1.0, 0.0]"#,
        r#""translation": [0.0, 2.0, 0.0]"#,
    );
    assert_eq!(writes.follow(&mut scene), 1);

    let bones = scene.world.mesh(hero).unwrap().skeleton.bones.clone();
    assert!(!bones.contains(&decoy), "the decoy is no bone");
    assert_eq!(scene.world.parent_id(decoy), Some(bones[1]), "it hangs on");
}

#[test]
fn destroying_a_child_takes_it_out_of_its_parents_children() {
    let mut scene = Scene::new();
    let parent = scene.add_entity("Parent".to_string());
    let child = scene.add_entity("Child".to_string());
    scene.set_parent(child, Some(parent)).unwrap();

    scene.destroy_entity(child);

    assert!(scene.world.children(parent).is_empty());
    assert_dead_and_unlisted(&scene, &[child]);
}

#[test]
fn a_bone_gone_with_its_ancestors_is_warned_about_and_leaves_its_child_on_the_rig() {
    let mut scene = Scene::new();
    let rig = armature_owner(&mut scene);
    scene.sync_skeleton(rig);
    let hat = hang_gun(&mut scene, rig, "head");
    hang_gun(&mut scene, rig, "head");
    let pistol = hang_gun(&mut scene, rig, "hips");

    let attachments = scene.take_skeleton(rig);
    assert_eq!(scene.bone_ids().len(), 0, "the bones came down");
    assert_eq!(scene.world.parent_id(hat), Some(rig), "parked on the rig");
    // The new rig keeps only the root, renamed: `head`'s whole chain is gone.
    let mut skin = scene.world.mesh(rig).unwrap().skin.clone().unwrap();
    skin.names = vec!["pelvis".to_string()];
    skin.local_bind.truncate(1);
    skin.parents.truncate(1);
    skin.bind_global.truncate(1);
    skin.inverse_bind.truncate(1);
    skin.joint_nodes.truncate(1);
    scene.world.mesh_mut(rig).unwrap().skin = Some(skin);

    let missing = scene.restore_skeleton(rig, attachments);
    assert_eq!(missing, ["hips", "head"], "each bone gone, once");
    assert_eq!(scene.world.parent_id(hat), Some(rig));
    assert_eq!(scene.world.parent_id(pistol), Some(rig));
    assert_eq!(scene.bone_ids().len(), 1, "the new rig's one bone");
}
