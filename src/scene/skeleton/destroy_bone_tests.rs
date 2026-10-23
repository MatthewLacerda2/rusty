//! Destroying one bone takes its subtree, Unity's way (#1273): no bone of the rig
//! is left a root posed as a child, and the missing slots ride with their parent bone.

use std::cell::RefCell;
use std::rc::Rc;

use super::fixture::{armature_owner, armature_skin, hero, reload};
use crate::scene::Scene;

/// An armature rig with a `Gun` on `head`; returns the rig, its bones and the gun.
fn rig_with_gun(scene: &mut Scene) -> (u32, Vec<u32>, u32) {
    let rig = armature_owner(scene);
    scene.sync_skeleton(rig);
    let bones = scene.world.mesh(rig).unwrap().skeleton.ids();
    let gun = scene.add_entity("Gun".to_string());
    scene.set_parent(gun, Some(bones[2])).unwrap();
    (rig, bones, gun)
}

#[test]
fn destroying_a_bone_takes_its_descendant_bones_and_what_hangs_from_them() {
    let mut scene = Scene::new();
    let (rig, bones, gun) = rig_with_gun(&mut scene);
    scene.destroy_entity(bones[1]); // spine

    for dead in [bones[1], bones[2], gun] {
        assert!(!scene.world.contains(dead), "entity {dead} went with spine");
    }
    assert_eq!(scene.world.parent_id(bones[0]), Some(rig), "hips stays");
    let roots: Vec<u32> = scene
        .entity_ids()
        .into_iter()
        .filter(|&id| scene.world.parent_id(id).is_none())
        .collect();
    assert_eq!(roots, [rig], "no bone of the rig is left a root");
    assert_eq!(scene.bone_ids().len(), 1);
}

#[test]
fn a_destroyed_bone_stays_missing_and_a_frame_skins_it() {
    let mut scene = Scene::new();
    let (rig, bones, _) = rig_with_gun(&mut scene);
    scene.destroy_entity(bones[1]);
    let count = scene.entity_count();
    scene.sync_skeletons();
    assert_eq!(
        scene.entity_count(),
        count,
        "a missing bone is not respawned"
    );
    let slots = scene.world.mesh(rig).unwrap().skeleton.bones.clone();
    assert_eq!(
        slots,
        [Some(bones[0]), None, None],
        "spine and head are missing"
    );

    // The next tick runs the whole frame over the half-gone rig without a panic.
    let scene = Rc::new(RefCell::new(scene));
    let mut game = game_over(Rc::clone(&scene));
    game.set_playing(true);
    game.tick(1.0 / 60.0);
    let palette = scene.borrow().world.mesh(rig).unwrap().pose_palette.clone();
    let bind = armature_skin().bind_palette();
    assert_eq!(palette.len(), 3);
    for (got, want) in palette.iter().zip(bind.iter()) {
        assert!(got.abs_diff_eq(*want, 1e-5), "got {got:?} want {want:?}");
    }
}

#[test]
fn a_missing_bone_rides_with_its_surviving_parent() {
    let mut scene = Scene::new();
    let (rig, bones, _) = rig_with_gun(&mut scene);
    scene.destroy_entity(bones[1]); // spine and head
    let mut hips = scene.world.transform_mut(bones[0]).unwrap();
    hips.rotation = glam::Quat::from_rotation_z(1.0);
    hips.position += glam::Vec3::X;
    drop(hips);
    scene.build_skin_palettes();
    let palette = scene.world.mesh(rig).unwrap().pose_palette.clone();
    let bind = armature_skin().bind_palette();
    assert!(!palette[0].abs_diff_eq(bind[0], 1e-3), "hips moved");
    for slot in [1, 2] {
        let (got, hips) = (palette[slot], palette[0]);
        assert!(
            got.abs_diff_eq(hips, 1e-5),
            "slot {slot} rides rigidly with hips"
        );
    }
}

#[test]
fn a_reload_rebuilds_a_destroyed_bone_from_the_model() {
    let mut scene = Scene::new();
    let hero = hero(&mut scene, "1273_reload");
    let joint1 = scene.find_bone(hero, "Joint1").unwrap();
    scene.destroy_entity(joint1);
    assert_eq!(scene.bone_ids().len(), 1);

    // A real save and load: `to_scene_data` → JSON → `apply_scene_data`.
    let loaded = reload(&scene);
    let hero = loaded.find_entity_by_name("Hero").unwrap();
    assert_eq!(loaded.bone_ids().len(), 2, "the rig is whole again");
    assert!(loaded.find_bone(hero, "Joint1").is_some());
}

/// A game over `scene`.
fn game_over(scene: Rc<RefCell<Scene>>) -> crate::app::GameWorld {
    let nav = crate::navigation::NavigationGraph::new(-1.0, 1.0, -1.0, 1.0, 1.0);
    crate::app::GameWorld::new(
        scene,
        Rc::new(RefCell::new(crate::core::input::InputState::new())),
        Rc::new(RefCell::new(nav)),
        Rc::new(RefCell::new(crate::scripting::ConsoleLogs::new())),
    )
}
