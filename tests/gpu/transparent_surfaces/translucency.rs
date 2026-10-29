//! Thin translucency (#1523, HDRP's thin *Translucent* type): an opaque leaf-green
//! card with the sun behind it is dark on the side the camera sees, until its
//! transmission lets the sunlight through.

use glam::{Quat, Vec3};
use rusty::components::MaterialAsset;
use rusty::scene::Camera;

use super::{mean_in, shot, slab, stage_with, SIZE};

/// The card facing the camera under a black sky, the sun shining through it toward
/// the camera, its material letting `transmission` through.
fn card(transmission: f32) -> rusty::scene::Scene {
    let behind = Quat::from_rotation_y(std::f32::consts::PI) * Quat::from_rotation_x(-0.3);
    let mut scene = stage_with(behind, |_| {});
    crate::gpu::black_sky(&mut scene);
    scene.ambient_intensity = 0.0;
    let leaf = MaterialAsset {
        base_color: [0.4, 0.8, 0.2],
        roughness: 1.0,
        transmission,
        ..MaterialAsset::default()
    };
    slab(
        &mut scene,
        "Card",
        Vec3::ZERO,
        Vec3::new(4.0, 4.0, 0.01),
        leaf,
    );
    scene
}

#[test]
fn a_backlit_translucent_card_glows_through() {
    let cam = Camera::new(Vec3::new(0.0, 0.0, 3.0), -90.0, 0.0);
    let Some(dark) = shot(&card(0.0), &cam, "translucency_off") else {
        return;
    };
    let lit = shot(&card(1.0), &cam, "translucency_on").unwrap();
    let middle = (SIZE / 4, 3 * SIZE / 4);
    let (dark, lit) = (
        mean_in(&dark, middle, middle),
        mean_in(&lit, middle, middle),
    );
    eprintln!("[1523] backlit card: opaque {dark:.1}, translucent {lit:.1}");
    assert!(
        lit > dark * 1.5 + 10.0,
        "the sun comes through: {lit:.1} vs {dark:.1}"
    );
}
