//! A plane whose material draws both faces (#1615) is shaded from the side it is seen,
//! its normal turned toward the camera, and the Fresnel angle is `|N.V|`: seen from
//! behind, it reflects what its front does, not a grazing ~50% of the sky.

use std::f32::consts::FRAC_PI_2;

use glam::{Quat, Vec3};
use rusty::components::{MaterialAsset, RenderFace};
use rusty::scene::authoring::Primitive;

use super::{black_plastic, reflectance, scene};

/// A 3x3 plane over the frame's centre (the corner stays sky), its normal (+Y)
/// turned toward the camera (`+Z`) by `angle`, or away from it by `-angle`. `None`
/// when no adapter rendered it.
fn plane(label: &str, angle: f32) -> Option<f64> {
    let both = MaterialAsset {
        render_face: RenderFace::Both,
        ..black_plastic(0.5)
    };
    let (mut scene, id) = scene(Primitive::Plane, both);
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
