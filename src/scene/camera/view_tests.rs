//! The main view's pose (#1619): `Camera.*` writes move the main camera entity, and
//! the view camera mirrors it.

use super::*;
use crate::scene::authoring::default_camera;

fn scene_with_camera() -> (Scene, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Cam".to_string());
    scene.world.set_camera(id, Some(default_camera()));
    (scene, id)
}

fn assert_pose(cam: &Camera, position: Vec3, yaw: f32, pitch: f32) {
    assert!(
        cam.position.abs_diff_eq(position, 1e-4),
        "{:?}",
        cam.position
    );
    assert!((cam.yaw - yaw).abs() < 1e-3, "yaw {}", cam.yaw);
    assert!((cam.pitch - pitch).abs() < 1e-3, "pitch {}", cam.pitch);
}

#[test]
fn look_rotation_inverts_the_entity_pose() {
    for (yaw, pitch) in [(0.0, 0.0), (-90.0, 0.0), (135.0, 30.0), (-45.0, -89.0)] {
        let (mut scene, id) = scene_with_camera();
        scene.world.transform_mut(id).expect("t").rotation = look_rotation(yaw, pitch);
        let base = Camera::new(Vec3::ZERO, 0.0, 0.0);
        let cam = game_camera_from_scene(&base, &scene);
        assert_pose(&cam, Vec3::ZERO, yaw, pitch);
        let forward = Camera::new(Vec3::ZERO, yaw, pitch).forward();
        assert!(cam.forward().abs_diff_eq(forward, 1e-5));
    }
}

#[test]
fn setting_the_view_moves_the_main_camera_entity() {
    let (mut scene, id) = scene_with_camera();
    // A parent offset: the pose is the entity's world one, not its local.
    let rig = scene.add_entity("Rig".to_string());
    scene.world.transform_mut(rig).expect("t").position = Vec3::new(10.0, 0.0, 0.0);
    scene.set_parent(id, Some(rig)).expect("parent");
    let mut camera = Camera::new(Vec3::ZERO, 0.0, 0.0);

    set_view_position(&mut camera, &mut scene, Vec3::new(1.0, 2.0, 3.0));
    set_view_orientation(&mut camera, &mut scene, 30.0, 120.0);

    assert_pose(&camera, Vec3::new(1.0, 2.0, 3.0), 30.0, MAX_PITCH);
    let world = scene.world.local_to_world(id).col(3).truncate();
    assert!(world.abs_diff_eq(Vec3::new(1.0, 2.0, 3.0), 1e-4));
    let view = game_camera_from_scene(&Camera::new(Vec3::ZERO, 0.0, 0.0), &scene);
    assert_pose(&view, Vec3::new(1.0, 2.0, 3.0), 30.0, MAX_PITCH);
}

#[test]
fn the_view_camera_mirrors_the_entity_and_keeps_its_own_without_one() {
    let (mut scene, id) = scene_with_camera();
    scene.world.transform_mut(id).expect("t").position = Vec3::new(4.0, 5.0, 6.0);
    let mut camera = Camera::new(Vec3::ZERO, 10.0, 0.0);
    sync_pose_from_scene(&mut camera, &scene);
    assert_pose(&camera, Vec3::new(4.0, 5.0, 6.0), -90.0, 0.0);

    // No camera entity: the free camera is the view, untouched by the sync and
    // moved by the setters alone.
    let mut empty = Scene::new();
    let mut free = Camera::new(Vec3::ONE, 10.0, 5.0);
    sync_pose_from_scene(&mut free, &empty);
    set_view_orientation(&mut free, &mut empty, 20.0, -5.0);
    assert_pose(&free, Vec3::ONE, 20.0, -5.0);
}
