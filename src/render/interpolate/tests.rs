//! The drawn pose between two ticks (#1877): blended while moving, never a jump,
//! and the live pose whenever the sim is not on the tick.

use glam::Vec3;

use super::*;
use crate::components::CameraComponent;
use crate::scene::authoring::{create_entity, Primitive};

fn scene_with_box() -> (Scene, u32) {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Box", Some(Primitive::Box));
    (scene, id)
}

fn place(scene: &mut Scene, id: u32, x: f32) {
    scene.world.transform_mut(id).unwrap().position = Vec3::new(x, 0.0, 0.0);
}

fn drawn_x(poses: &DrawPoses, scene: &Scene, id: u32) -> f32 {
    poses.world(scene, id).w_axis.x
}

/// Two ticks recorded: the box at 0, then at `x`.
fn moved(x: f32) -> (Scene, u32, DrawPoses) {
    let (mut scene, id) = scene_with_box();
    let mut poses = DrawPoses::default();
    poses.record(&scene);
    place(&mut scene, id, x);
    poses.record(&scene);
    (scene, id, poses)
}

#[test]
fn a_moving_entity_is_drawn_between_its_last_two_ticks() {
    let (scene, id, mut poses) = moved(1.0);
    poses.set_alpha(0.25);
    assert!((drawn_x(&poses, &scene, id) - 0.25).abs() < 1e-5);
    poses.set_alpha(0.0);
    assert!(
        drawn_x(&poses, &scene, id).abs() < 1e-5,
        "alpha 0 is the tick before"
    );
    poses.set_alpha(1.0);
    assert_eq!(poses.world(&scene, id), scene.world.local_to_world(id));
}

#[test]
fn a_jump_is_drawn_where_it_landed() {
    let (scene, id, mut poses) = moved(JUMP_DISTANCE + 0.5);
    poses.set_alpha(0.5);
    assert_eq!(poses.world(&scene, id), scene.world.local_to_world(id));
}

#[test]
fn a_spawn_and_the_first_tick_are_drawn_where_they_are() {
    let (mut scene, first) = scene_with_box();
    let mut poses = DrawPoses::default();
    poses.set_alpha(0.5);
    poses.record(&scene); // the first tick: nothing before it
    assert_eq!(
        poses.world(&scene, first),
        scene.world.local_to_world(first)
    );
    let spawned = create_entity(&mut scene, "Late", Some(Primitive::Box));
    place(&mut scene, spawned, 1.0);
    poses.record(&scene);
    assert_eq!(
        poses.world(&scene, spawned),
        scene.world.local_to_world(spawned)
    );
}

#[test]
fn an_entity_that_stops_is_drawn_at_its_pose_and_reposed_once() {
    let (mut scene, id, mut poses) = moved(1.0);
    poses.set_alpha(0.5);
    assert!(
        poses.repose(&scene).contains(&id),
        "moving: re-posed every frame"
    );
    poses.record(&scene); // a tick that moved nothing
    assert_eq!(poses.world(&scene, id), scene.world.local_to_world(id));
    assert!(
        poses.repose(&scene).contains(&id),
        "drawn blended last frame: once more"
    );
    assert!(poses.repose(&scene).is_empty());
    place(&mut scene, id, 1.5);
    poses.record(&scene);
    poses.clear(); // Pause, Stop, edit mode
    assert_eq!(poses.world(&scene, id), scene.world.local_to_world(id));
}

#[test]
fn another_scene_is_never_blended_and_keeps_what_is_owed() {
    let (scene, id, mut poses) = moved(1.0);
    poses.set_alpha(0.5);
    assert!(poses.repose(&scene).contains(&id));
    let (other, other_id) = scene_with_box();
    assert_eq!(
        poses.world(&other, other_id),
        other.world.local_to_world(other_id)
    );
    assert!(poses.repose(&other).is_empty());
    poses.clear();
    assert!(
        poses.repose(&scene).contains(&id),
        "the sim's scene still re-poses it"
    );
}

#[test]
fn the_main_camera_is_drawn_blended() {
    let mut scene = Scene::new();
    let id = create_entity(&mut scene, "Eye", None);
    scene.world.set_camera(id, Some(CameraComponent::default()));
    let mut poses = DrawPoses::default();
    poses.record(&scene);
    place(&mut scene, id, 1.0);
    poses.record(&scene);
    poses.set_alpha(0.5);
    let base = Camera::new(Vec3::ZERO, 0.0, 0.0);
    let camera = poses.game_camera(&base, &scene);
    assert!(
        (camera.position.x - 0.5).abs() < 1e-5,
        "{:?}",
        camera.position
    );
    poses.set_alpha(1.0);
    let live = crate::scene::game_camera_from_scene(&base, &scene);
    assert_eq!(poses.game_camera(&base, &scene).position, live.position);
}

#[test]
fn a_turn_past_a_quarter_is_a_jump() {
    let a = Mat4::IDENTITY;
    let quarter = Mat4::from_rotation_y(JUMP_TURN * 0.9);
    let half = Mat4::from_rotation_y(std::f32::consts::PI * 0.75);
    assert!(!jumped(a, quarter));
    assert!(jumped(a, half));
    assert!(jumped(a, Mat4::from_translation(Vec3::splat(f32::NAN))));
}
