//! A dielectric's grazing reflection follows `KHR_materials_specular` (#1998): its
//! `specular` is the F90 the environment BRDF rises to, so at 0 a smooth dielectric
//! shows no rim of sky at all, and at the default 1 it keeps Schlick's.

use glam::{Quat, Vec3};
use rusty::components::MaterialAsset;
use rusty::scene::authoring::Primitive;

use super::{black_plastic, reflectance, scene};

/// A narrow, long slab seen at 80° from its normal: the frame's centre lands on it and its corners on the sky, so its
/// reflection weight there is the grazing one. `None` when no adapter rendered it.
fn grazing(label: &str, material: MaterialAsset) -> Option<f64> {
    let (mut scene, id) = scene(Primitive::Box, material);
    {
        let mut transform = scene.world.transform_mut(id).unwrap();
        transform.scale = Vec3::new(8.0, 1.0, 0.1);
        transform.rotation = Quat::from_rotation_y(80f32.to_radians());
    }
    reflectance(label, &scene)
}

#[test]
fn a_dielectric_at_specular_zero_reflects_nothing_at_grazing_angles() {
    let Some(full) = grazing("grazing_full", black_plastic(0.05)) else {
        return;
    };
    let half = MaterialAsset {
        specular: 0.5,
        ..black_plastic(0.05)
    };
    let half = grazing("grazing_half", half).expect("the adapter rendered once");
    let none = MaterialAsset {
        specular: 0.0,
        ..black_plastic(0.05)
    };
    let none = grazing("grazing_none", none).expect("the adapter rendered once");
    // Head-on 4%; at 80° Schlick's rim takes it to about half the sky.
    assert!(full > 0.25, "the default keeps its grazing rim: {full:.4}");
    assert!(
        (0.3 * full..0.7 * full).contains(&half),
        "half the specular, about half the rim: {half:.4} of {full:.4}"
    );
    assert!(none < 0.005, "specular 0 reflects nothing: {none:.4}");
}
