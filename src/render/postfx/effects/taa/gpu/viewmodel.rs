//! A viewmodel under TAA on lavapipe (#1702): it does not swim against a turning
//! world, and its silhouette reprojects with its own motion, not the world's behind
//! it, even where the world is nearer its own camera (motion blur's stack order).

use glam::{Quat, Vec3};

use super::fixture::{cube, diff, eye, frame, fresh_frame, renderer, settle, taa_scene, view};
use crate::components::CameraComponent;
use crate::render::motion::gpu_fixture::{H, W};
use crate::render::postfx::effects::motion_blur::stack_order_gpu_tests::against_a_wall;
use crate::scene::{ClearFlags, Scene};

/// The fixture scene, a cube 6 m out to the right, and a `DepthOnly` overlay camera
/// holding a gun cube 3 m ahead (the motion-blur viewmodel stack). Returns the
/// scene and the overlay camera.
fn viewmodel(taa: bool) -> (Scene, u32) {
    let (mut scene, _) = taa_scene(taa, false);
    cube(&mut scene, Vec3::new(2.0, 0.0, -6.0));
    let overlay = scene.add_entity("ViewmodelCamera".to_string());
    let lens = CameraComponent {
        render_order: 1,
        clear_flags: ClearFlags::DepthOnly,
        ..Default::default()
    };
    scene.world.set_camera(overlay, Some(lens));
    let gun = cube(&mut scene, Vec3::new(0.0, 0.0, -3.0));
    scene.set_parent(gun, Some(overlay)).unwrap();
    (scene, overlay)
}

#[test]
fn gpu_a_viewmodel_does_not_swim_against_the_turning_world() {
    let Some(mut r) = renderer() else { return };
    let (mut scene, overlay) = viewmodel(true);
    let mut v = view(&r);
    let mut last = Vec::new();
    for step in 0..8 {
        let yaw = step as f32 * 3.0;
        scene.world.transform_mut(overlay).unwrap().rotation =
            Quat::from_rotation_y(-yaw.to_radians());
        last = frame(&mut r, &mut v, &scene, &eye(yaw));
    }
    let (mut plain, overlay) = viewmodel(false);
    plain.world.transform_mut(overlay).unwrap().rotation =
        Quat::from_rotation_y(-21f32.to_radians());
    let sharp = fresh_frame(&mut r, &plain, &eye(21.0));
    let gun = (29..=35).flat_map(|x| (21..=27).map(move |y| (x, y)));
    let worst = gun.map(|p| diff(&last, &sharp, p)).max().unwrap();
    eprintln!("[taa] the gun under a turning camera differs from no TAA by {worst}");
    assert!(worst <= 6, "the gun swam or smeared: {worst}");
}

/// Motion blur's wall scene with the world camera running TAA: a green gun 0.6 m
/// from its camera over a blue wall 0.35 m from the world's, the gun's right side
/// on the wall.
fn wall_scene() -> Scene {
    let (mut scene, world) = against_a_wall(false);
    scene.world.camera_mut(world).unwrap().taa_active = true;
    scene
}

/// Whether pixel `(x, y)` of `frame` is mostly the green gun.
fn green(frame: &[u8], (x, y): (u32, u32)) -> bool {
    let i = ((y * W + x) * 4) as usize;
    frame[i + 1] > frame[i + 2].saturating_add(40)
}

#[test]
fn gpu_the_guns_edge_over_a_nearer_wall_does_not_smear_under_taa() {
    let Some(mut r) = renderer() else { return };
    let scene = wall_scene();
    // The world camera turns 3 degrees a frame; the gun camera holds still.
    let mut v = view(&r);
    let mut turned = Vec::new();
    for step in 0..8 {
        turned = frame(&mut r, &mut v, &scene, &eye(step as f32 * 3.0));
    }
    let mut still = view(&r);
    let reference = settle(&mut r, &mut still, &scene, &eye(21.0), 8);
    // Every gun pixel of the settled frame, its silhouette included.
    let pixels = (0..H).flat_map(|y| (0..W).map(move |x| (x, y)));
    let gun: Vec<_> = pixels.filter(|&p| green(&reference, p)).collect();
    assert!(!gun.is_empty(), "the gun is on screen");
    let worst = gun
        .iter()
        .map(|&p| diff(&turned, &reference, p))
        .max()
        .unwrap();
    eprintln!("[taa] the gun's edge over a nearer wall differs by {worst} while turning");
    assert!(
        worst <= 24,
        "the wall's motion smeared the gun's edge: {worst}"
    );
}
