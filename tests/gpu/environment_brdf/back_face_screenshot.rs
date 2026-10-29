//! The forward pass draws both sides of a surface and never turns its normal toward
//! the camera, so the Fresnel angle is `|N.V|`: seen from behind, a one-sided plane
//! reflects what its front does, not a grazing ~50% of the sky.

use std::f32::consts::FRAC_PI_2;

use glam::{Quat, Vec3};
use rusty::scene::authoring::Primitive;

use super::{black_plastic, reflectance, scene};

/// A 3x3 plane over the frame's centre (the corner stays sky), its normal (+Y)
/// turned toward the camera (`+Z`) by `angle`, or away from it by `-angle`. `None`
/// when no adapter rendered it.
fn plane(label: &str, angle: f32) -> Option<f64> {
    let (mut scene, id) = scene(Primitive::Plane, black_plastic(0.5));
    {
        let mut transform = scene.world.transform_mut(id).unwrap();
        transform.rotation = Quat::from_rotation_x(angle);
        transform.scale = Vec3::splat(0.2);
    }
    reflectance(label, &scene)
}

#[test]
fn a_plane_seen_from_behind_reflects_what_its_front_does() {
    let Some(front) = plane("plane_front", FRAC_PI_2) else {
        return;
    };
    let back = plane("plane_back", -FRAC_PI_2).expect("the adapter rendered once");
    assert!(front < 0.06, "the front reflects its ~4% F0: {front:.4}");
    assert!(
        (back - front).abs() < 0.5 * front,
        "the back face reflects {back:.4} of the sky against the front's {front:.4}"
    );
}
