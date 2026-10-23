//! A re-written skinned model rebuilds the rig and bones of the entities built from
//! it (#1252), keeping the bone overrides and what hangs from a bone.

use glam::Vec3;

use super::fixture::{animated_model, armature_owner};
use crate::asset::fixtures_anim::ANIMATED_GLTF;
use crate::scene::asset_instance::instantiate_asset;
use crate::scene::model_writes::ModelWrites;
use crate::scene::Scene;

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

#[test]
fn a_changed_bind_pose_rebuilds_the_rig_and_keeps_what_was_authored() {
    let mut scene = Scene::new();
    let (hero, path) = hero(&mut scene, "reimport_bind");
    let root = scene.find_bone(hero, "Joint0").unwrap();
    scene.world.transform_mut(root).unwrap().position = Vec3::new(0.5, 0.0, 0.0);
    let gun = hang_gun(&mut scene, hero, "Joint1");
    let old_palette = scene.world.mesh(hero).unwrap().bind_palette.clone();
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
}

#[test]
fn a_removed_bone_hands_what_hung_from_it_to_its_parent_bone() {
    let mut scene = Scene::new();
    let (hero, path) = hero(&mut scene, "reimport_removed");
    let gun = hang_gun(&mut scene, hero, "Joint1");
    let mut writes = ModelWrites::default();

    rewrite(
        &path,
        r#""joints": [ 1, 2 ], "inverseBindMatrices": 3"#,
        r#""joints": [ 1 ]"#,
    );
    assert_eq!(writes.follow(&mut scene), 1);

    let bones = scene.world.mesh(hero).unwrap().skeleton.bones.clone();
    assert_eq!(bones.len(), 1, "the rig lost Joint1");
    assert_eq!(scene.world.parent_id(gun), Some(bones[0]));
    assert_eq!(parent_name(&scene, gun), "Joint0");
}

#[test]
fn a_bone_gone_with_its_ancestors_is_warned_about_and_leaves_its_child_on_the_rig() {
    let mut scene = Scene::new();
    let rig = armature_owner(&mut scene);
    scene.sync_skeleton(rig);
    let hat = hang_gun(&mut scene, rig, "head");
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
    assert_eq!(missing, ["hips", "head"], "each bone gone is warned about");
    assert_eq!(scene.world.parent_id(hat), Some(rig));
    assert_eq!(scene.world.parent_id(pistol), Some(rig));
    assert_eq!(scene.bone_ids().len(), 1, "the new rig's one bone");
}
