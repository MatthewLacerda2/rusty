//! What hangs from a bone is drawn at the tick (#1877): a gun on a moving hand
//! bone, with the skinned owner's root still, never drifts off the hand.

use glam::Vec3;

use super::*;
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::skeleton::rig::armature_owner;

#[test]
fn a_mesh_on_a_moving_bone_is_drawn_at_the_tick() {
    let mut scene = Scene::new();
    let rig = armature_owner(&mut scene);
    scene.sync_skeleton(rig);
    let hand = scene.find_bone(rig, "head").unwrap();
    let gun = create_entity(&mut scene, "Gun", Some(Primitive::Box));
    scene.set_parent(gun, Some(hand)).unwrap();
    let mut poses = DrawPoses::default();
    poses.record(&scene);
    // The animation moves the bone; the rig's root stays where it is.
    scene.world.transform_mut(hand).unwrap().position += Vec3::new(0.0, 0.0, 0.5);
    poses.record(&scene);
    poses.set_alpha(0.5);
    assert!(on_bone(&scene, gun) && on_bone(&scene, hand) && !on_bone(&scene, rig));
    assert_eq!(poses.world(&scene, gun), scene.world.local_to_world(gun));
    assert!(poses.repose(&scene).is_empty(), "nothing drawn blended");
}

#[test]
fn a_teleport_between_ticks_is_drawn_where_it_landed() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Box", Some(Primitive::Box));
    let mut poses = DrawPoses::default();
    poses.record(&scene);
    scene.world.transform_mut(id).unwrap().position.x = 1.0;
    poses.record(&scene);
    poses.set_alpha(0.5);
    // The console moves it far between ticks: no smear across the map.
    scene.world.transform_mut(id).unwrap().position.x = 40.0;
    assert_eq!(poses.world(&scene, id), scene.world.local_to_world(id));
}
