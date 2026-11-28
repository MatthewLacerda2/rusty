use glam::Vec3;

use super::*;
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::skeleton::rig::armature_owner;

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
    scene.world.transform_mut(id).unwrap().position.x = 40.0;
    assert_eq!(poses.world(&scene, id), scene.world.local_to_world(id));
}

#[test]
fn a_pose_written_between_ticks_is_not_blended_from_before_the_write() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Box", Some(Primitive::Box));
    scene.world.transform_mut(id).unwrap().position.x = 1.0;
    let mut poses = DrawPoses::default();
    poses.record(&scene);
    scene.world.transform_mut(id).unwrap().position.x = 2.0;
    poses.record(&scene);
    poses.set_alpha(0.25);
    // The console writes after the frame's ticks; the next frame settles, then ticks.
    scene.world.transform_mut(id).unwrap().position.x = 40.0;
    poses.settle(&scene);
    assert_eq!(
        poses.world(&scene, id).w_axis.x,
        40.0,
        "drawn where it was set"
    );
    poses.record(&scene);
    for alpha in [0.0, 0.25, 0.99] {
        poses.set_alpha(alpha);
        assert_eq!(poses.world(&scene, id).w_axis.x, 40.0, "no slide from x 2");
    }
    scene.world.transform_mut(id).unwrap().position.x = 41.0;
    poses.record(&scene);
    poses.set_alpha(0.5);
    let x = poses.world(&scene, id).w_axis.x;
    assert!(
        (x - 40.5).abs() < 1e-4,
        "the tick after blends from the write: {x}"
    );
}
