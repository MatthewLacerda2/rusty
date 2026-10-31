//! A capture sees through its own lens (#1615): the scene's cameras — a 45° main
//! camera, an overlay drawn over it — change nothing in a baked cube, and a cube
//! face's up vector never reaches a scene camera.

use glam::{Quat, Vec3};

use super::texture_cube_tests::patched_room;
use crate::components::ClearFlags;
use crate::scene::authoring::default_camera;
use crate::scene::{camera_for_entity, Camera, ReflectionProbe, Scene};

const RES: u32 = 32;

/// A screen camera at `pos`, turned off the axes, drawing in `order` (an overlay
/// above 0, clearing depth only).
fn add_camera(scene: &mut Scene, name: &str, pos: Vec3, fov: f32, order: i32) -> u32 {
    let id = scene.add_entity(name.to_string());
    let mut c = default_camera();
    (c.fov, c.render_order) = (fov, order);
    if order > 0 {
        c.clear_flags = ClearFlags::DepthOnly;
    }
    scene.world.set_camera(id, Some(c));
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.rotation) = (pos, Quat::from_rotation_y(0.7));
    id
}

#[test]
fn gpu_a_scene_s_cameras_leave_a_baked_cube_unchanged() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let probe = ReflectionProbe::new(Vec3::ZERO, Vec3::splat(5.0));
    let bare = r.bake_reflection(&patched_room(), &probe, RES);
    let mut scene = patched_room();
    add_camera(&mut scene, "Main", Vec3::new(2.0, 1.0, 3.0), 45.0, 0);
    add_camera(&mut scene, "Overlay", Vec3::new(-3.0, 0.0, 1.0), 30.0, 1);
    let with_cameras = r.bake_reflection(&scene, &probe, RES);
    assert!(
        bare == with_cameras,
        "the scene's cameras changed the baked cube"
    );
}

/// A scene camera starts upright whatever `base` carries: a face camera's up (-Z on
/// the +Y face) would leave a render-texture camera's basis degenerate.
#[test]
fn a_scene_camera_is_upright_whatever_its_base() {
    let mut scene = Scene::new();
    let id = add_camera(&mut scene, "Cam", Vec3::ZERO, 60.0, 0);
    let mut base = Camera::new(Vec3::ZERO, 0.0, 90.0);
    base.up = Vec3::NEG_Z;
    let cam = camera_for_entity(&base, &scene, id).unwrap();
    assert_eq!(cam.up, Vec3::Y);
    assert!(cam.build_view_projection(1.0).is_finite());
}
