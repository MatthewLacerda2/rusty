//! The camera stack (#93) and render-texture cameras (#430).

use super::*;
use crate::components::{CameraComponent, ClearFlags, Projection, RenderTarget};
use glam::Vec3;

fn cam_component(order: i32, mask: u32, flags: ClearFlags) -> CameraComponent {
    CameraComponent {
        active: true,
        fov: 60.0,
        near: 0.2,
        far: 500.0,
        culling_mask: mask,
        render_order: order,
        clear_flags: flags,
        motion_blur_active: false,
        motion_blur_samples: 0,
        fxaa_active: true,
        projection: Default::default(),
        target_texture: None,
    }
}

fn add_camera(scene: &mut Scene, name: &str, comp: CameraComponent, at: Vec3) -> u32 {
    let id = scene.add_entity(name.to_string());
    scene.world.set_camera(id, Some(comp));
    scene.world.transform_mut(id).expect("transform").position = at;
    id
}

#[test]
fn edit_mode_is_a_single_base_pass() {
    let base = Camera::new(Vec3::ZERO, 0.0, 0.0);
    let mut scene = Scene::new();
    let comp = cam_component(0, u32::MAX, ClearFlags::Skybox);
    add_camera(&mut scene, "Cam", comp, Vec3::ZERO);
    // Edit mode ignores the scene cameras entirely.
    assert_eq!(build_camera_stack(&base, &scene, false).len(), 1);
}

#[test]
fn play_mode_sorts_by_render_order_and_layers_lens() {
    let base = Camera::new(Vec3::new(1.0, 2.0, 3.0), 10.0, 20.0);
    let mut scene = Scene::new();
    // Authored out of order; the viewmodel (order 1) must come after the world.
    let view = cam_component(1, 0b10, ClearFlags::DepthOnly);
    add_camera(&mut scene, "View", view, Vec3::new(7.0, 0.0, 0.0));
    let world = cam_component(0, 0b01, ClearFlags::Skybox);
    add_camera(&mut scene, "World", world, Vec3::new(-9.0, 0.0, 0.0));

    let stack = build_camera_stack(&base, &scene, true);
    assert_eq!(stack.len(), 2);
    assert_eq!(stack[0].culling_mask, 0b01);
    assert_eq!(stack[0].clear_flags, ClearFlags::Skybox);
    assert_eq!(stack[1].culling_mask, 0b10);
    assert_eq!(stack[1].clear_flags, ClearFlags::DepthOnly);
    // The base pass keeps the caller's pose; the overlay renders from its own entity.
    assert_eq!(stack[0].position, base.position);
    assert_eq!(stack[0].yaw, base.yaw);
    assert_eq!(stack[1].position, Vec3::new(7.0, 0.0, 0.0));
    assert_eq!(stack[1].fov, 60.0);
}

#[test]
fn inactive_cameras_are_skipped_with_base_fallback() {
    let base = Camera::new(Vec3::ZERO, 0.0, 0.0);
    let mut scene = Scene::new();
    let mut off = cam_component(0, u32::MAX, ClearFlags::Skybox);
    off.active = false;
    add_camera(&mut scene, "Off", off, Vec3::ZERO);
    assert_eq!(build_camera_stack(&base, &scene, true).len(), 1);
}

#[test]
fn texture_cameras_leave_the_screen_stack_and_keep_their_own_pose() {
    let base = Camera::new(Vec3::new(1.0, 2.0, 3.0), 0.0, 0.0);
    let mut scene = Scene::new();
    add_camera(
        &mut scene,
        "Main",
        cam_component(0, u32::MAX, ClearFlags::Skybox),
        Vec3::ZERO,
    );
    let mut mini = cam_component(-5, u32::MAX, ClearFlags::Skybox);
    mini.target_texture = Some(RenderTarget::new("minimap", 128, 64));
    mini.projection = Projection::Orthographic { size: 20.0 };
    add_camera(&mut scene, "Minimap", mini, Vec3::new(0.0, 50.0, 0.0));

    // A lower render_order must not make the texture camera the screen's base.
    let stack = build_camera_stack(&base, &scene, true);
    assert_eq!(stack.len(), 1);
    assert_eq!(stack[0].position, base.position);

    let rts = texture_cameras(&base, &scene);
    assert_eq!(rts.len(), 1);
    assert_eq!(rts[0].target.path(), "rt:minimap");
    assert_eq!(rts[0].camera.position, Vec3::new(0.0, 50.0, 0.0));
    assert_eq!(
        rts[0].camera.projection,
        Projection::Orthographic { size: 20.0 }
    );
}

#[test]
fn only_texture_cameras_falls_back_to_the_base_screen_pass() {
    let base = Camera::new(Vec3::ZERO, 0.0, 0.0);
    let mut scene = Scene::new();
    let mut rt = cam_component(0, u32::MAX, ClearFlags::Skybox);
    rt.target_texture = Some(RenderTarget::new("pip", 64, 64));
    add_camera(&mut scene, "Pip", rt, Vec3::ZERO);
    let stack = build_camera_stack(&base, &scene, true);
    assert_eq!(stack.len(), 1);
    assert_eq!(stack[0].clear_flags, ClearFlags::Skybox);
}
