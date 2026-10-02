//! Bones are GameObjects (#453): spawning, re-binding, the palette build, lookup,
//! destroy, and the snapshot filter.

use glam::{Vec2, Vec3};

use super::fixture::{armature_owner, armature_skin, hero};
use crate::scene::{Camera, Scene};

fn names(scene: &Scene, ids: &[u32]) -> Vec<String> {
    ids.iter()
        .map(|&id| scene.world.name(id).unwrap().clone())
        .collect()
}

#[test]
fn instantiating_a_skinned_asset_spawns_its_skeleton_as_child_entities() {
    let mut scene = Scene::new();
    let hero = hero(&mut scene, "spawn");
    let bones = scene.world.mesh(hero).unwrap().skeleton.bones.clone();
    assert_eq!(names(&scene, &bones), ["Joint0", "Joint1"]);
    // The joint hierarchy is the entity hierarchy, rooted under the skinned entity.
    assert_eq!(scene.world.parent_id(bones[0]), Some(hero));
    assert_eq!(scene.world.parent_id(bones[1]), Some(bones[0]));
    // Joint1 rests one unit up its parent, as the glTF says.
    let rest = scene.world.transform(bones[1]).unwrap().position;
    assert!(rest.abs_diff_eq(Vec3::Y, 1e-6));
}

#[test]
fn the_rest_pose_skins_exactly_like_the_bind_palette_under_an_armature() {
    let mut scene = Scene::new();
    let rig = armature_owner(&mut scene);
    scene.sync_skeleton(rig);
    // Move the whole character: the palette is mesh-local, so it must not care.
    scene.world.transform_mut(rig).unwrap().position = Vec3::new(5.0, 0.0, -2.0);
    scene.build_skin_palettes();
    let mesh = scene.world.mesh(rig).unwrap();
    let bind = armature_skin().bind_palette();
    assert_eq!(mesh.pose_palette.len(), 3);
    for (got, want) in mesh.pose_palette.iter().zip(bind.iter()) {
        assert!(got.abs_diff_eq(*want, 1e-5), "got {got:?} want {want:?}");
    }
}

#[test]
fn moving_a_bone_moves_its_subtree_in_the_palette() {
    let mut scene = Scene::new();
    let rig = armature_owner(&mut scene);
    scene.sync_skeleton(rig);
    let spine = scene.find_bone(rig, "spine").unwrap();
    scene.world.transform_mut(spine).unwrap().position = Vec3::new(0.0, 3.0, 0.0);
    scene.build_skin_palettes();
    let palette = scene.world.mesh(rig).unwrap().pose_palette.clone();
    // hips is untouched; spine and head both moved by the +2 Y (in armature space).
    assert!(palette[0].abs_diff_eq(glam::Mat4::IDENTITY, 1e-5));
    assert!(!palette[1].abs_diff_eq(glam::Mat4::IDENTITY, 1e-3));
    assert!(palette[1].abs_diff_eq(palette[2], 1e-5));
}

#[test]
fn sync_is_idempotent_and_rebinds_existing_bones_by_name() {
    let mut scene = Scene::new();
    let rig = armature_owner(&mut scene);
    scene.sync_skeleton(rig);
    let bones = scene.world.mesh(rig).unwrap().skeleton.bones.clone();
    let count = scene.entity_count();
    scene.sync_skeletons();
    assert_eq!(
        scene.entity_count(),
        count,
        "a bound skeleton spawns nothing"
    );
    // A propagation rebuild resets the binding; the same bones are found again.
    scene.world.mesh_mut(rig).unwrap().skeleton.bones.clear();
    scene.sync_skeletons();
    assert_eq!(scene.entity_count(), count);
    assert_eq!(scene.world.mesh(rig).unwrap().skeleton.bones, bones);
}

#[test]
fn bones_are_found_by_name_and_know_their_owner() {
    let mut scene = Scene::new();
    let rig = armature_owner(&mut scene);
    scene.sync_skeleton(rig);
    let head = scene.find_bone(rig, "head").unwrap();
    assert_eq!(scene.bone_owner(head), Some(rig));
    assert_eq!(scene.bone_owner(rig), None);
    assert_eq!(scene.find_bone(rig, "tail"), None);
    assert_eq!(scene.bone_ids().len(), 3);
}

#[test]
fn destroying_the_skinned_entity_takes_its_skeleton_and_attachments() {
    let mut scene = Scene::new();
    let rig = armature_owner(&mut scene);
    scene.sync_skeleton(rig);
    let gun = scene.add_entity("Gun".to_string());
    let head = scene.find_bone(rig, "head").unwrap();
    scene.set_parent(gun, Some(head)).unwrap();
    let bystander = scene.add_entity("Crate".to_string());
    scene.destroy_entity(rig);
    assert_eq!(scene.entity_ids(), vec![bystander]);
}

#[test]
fn the_snapshot_leaves_bones_out_unless_asked() {
    let mut scene = Scene::new();
    let rig = armature_owner(&mut scene);
    scene.sync_skeleton(rig);
    let read = |bones| {
        let v = crate::api::snapshot::world_value_with(
            &scene,
            &Camera::new(Vec3::ZERO, 0.0, 0.0),
            0,
            false,
            Vec2::new(800.0, 600.0),
            bones,
        );
        v["entities"].as_array().unwrap().clone()
    };
    assert_eq!(read(false).len(), 1);
    let all = read(true);
    assert_eq!(all.len(), 4);
    assert_eq!(all.iter().filter(|e| e["bone"] == true).count(), 3);
}
