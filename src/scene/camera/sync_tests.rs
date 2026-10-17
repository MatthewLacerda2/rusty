//! `sync_lens_from_scene`: the free camera gets its fov back when the scene camera
//! goes, and keeps a script's own fov otherwise (#997).

use super::*;
use crate::scene::authoring::default_camera;

#[test]
fn removing_the_scene_camera_restores_the_default_fov() {
    let mut scene = Scene::new();
    let id = scene.add_entity("Cam".to_string());
    let mut c = default_camera();
    c.fov = 70.0;
    scene.world.set_camera(id, Some(c));
    let mut camera = Camera::new(Vec3::ZERO, 0.0, 0.0);

    sync_lens_from_scene(&mut camera, &scene, true);
    assert_eq!(camera.fov, 70.0, "the scene camera drives the lens");

    scene.world.set_camera(id, None);
    sync_lens_from_scene(&mut camera, &scene, true);
    assert_eq!(camera.fov, DEFAULT_FOV, "its fov does not outlive it");
}

#[test]
fn a_free_camera_fov_survives_the_sync() {
    let scene = Scene::new();
    let mut camera = Camera::new(Vec3::ZERO, 0.0, 0.0);
    camera.fov = 60.0; // what `Camera.SetFov` does
    sync_lens_from_scene(&mut camera, &scene, true);
    sync_lens_from_scene(&mut camera, &scene, false);
    assert_eq!(camera.fov, 60.0);
}
