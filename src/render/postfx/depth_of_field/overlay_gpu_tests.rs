//! The viewmodel (#1445): a gun on a `DepthOnly` overlay camera, 0.7 m from it, stays
//! sharp while the world focuses 12 m out. Its circle of confusion is its own
//! camera's business, so the world's lens never reaches it, even though the overlay
//! cleared the depth buffer the world drew.

use glam::Vec3;

use super::gpu_fixture::{bokeh, change_around, cube, renderer, scene, shot, NEAR};
use crate::components::{CameraComponent, DepthOfField};
use crate::core::settings::quality::QualityPreset;
use crate::scene::{ClearFlags, Scene};

/// The layer only the gun camera draws.
const VIEWMODEL: u8 = 1;

/// Where the gun sits in front of its camera: low and right, clear of both cubes.
const GUN: Vec3 = Vec3::new(0.4, -0.25, -0.7);

/// The two-cube scene running `dof`, with the usual FPS stack: the world camera
/// skips the viewmodel layer, and a `DepthOnly` gun camera above it draws only that
/// layer, a small gun cube.
fn armed(dof: DepthOfField) -> Scene {
    let (mut scene, world) = scene(dof);
    scene.world.camera_mut(world).unwrap().culling_mask = !(1 << VIEWMODEL);
    let camera = scene.add_entity("GunCamera".to_string());
    let lens = CameraComponent {
        render_order: 1,
        clear_flags: ClearFlags::DepthOnly,
        culling_mask: 1 << VIEWMODEL,
        ..Default::default()
    };
    scene.world.set_camera(camera, Some(lens));
    let gun = cube(&mut scene, GUN);
    scene.world.transform_mut(gun).unwrap().scale = Vec3::splat(0.15);
    scene.world.set_layer(gun, VIEWMODEL);
    scene.set_parent(gun, Some(camera)).unwrap();
    scene
}

#[test]
fn gpu_a_viewmodel_stays_sharp_while_the_world_focuses_far() {
    let Some(mut r) = renderer(QualityPreset::High) else {
        return;
    };
    let off = shot(&mut r, &armed(DepthOfField::default()));
    let focused = shot(&mut r, &armed(bokeh(12.0)));
    let gun = change_around(&focused, &off, GUN, 14);
    let near = change_around(&focused, &off, NEAR, 12);
    eprintln!("[dof] focus far under a gun camera: gun changed {gun}, near cube {near}");
    assert!(near >= 16, "the world's near cube blurs ({near})");
    assert!(
        gun <= 2,
        "the gun 0.7 m from its camera stays sharp ({gun})"
    );
}
