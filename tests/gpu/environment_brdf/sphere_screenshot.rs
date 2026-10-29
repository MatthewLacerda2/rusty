//! A sphere filling the frame's centre: rough against smooth metal, and a
//! dielectric's F0.

use glam::Vec3;
use rusty::components::MaterialAsset;
use rusty::scene::authoring::Primitive;

use super::{black_plastic, reflectance, scene};

/// The sphere's reflection weight head-on. `None` when no adapter rendered it.
fn sphere(label: &str, material: MaterialAsset) -> Option<f64> {
    let (mut scene, id) = scene(Primitive::Sphere, material);
    scene.world.transform_mut(id).unwrap().scale = Vec3::splat(3.0);
    reflectance(label, &scene)
}

fn metal(roughness: f32) -> MaterialAsset {
    MaterialAsset {
        base_color: [1.0, 1.0, 1.0],
        metallic: 1.0,
        roughness,
        ..MaterialAsset::default()
    }
}

#[test]
fn rough_metal_reflects_a_dimmer_environment_not_a_black_one() {
    let Some(smooth) = sphere("smooth", metal(0.1)) else {
        return;
    };
    let rough = sphere("rough", metal(0.9)).expect("the adapter rendered once");
    // URP's `1 / (roughness^4 + 1)` (perceptual roughness) keeps about 60% at 0.9;
    // the old `1 - roughness` scale kept about 11%.
    assert!(
        smooth > 0.85,
        "smooth white metal reflects ~all of the sky: {smooth:.3}"
    );
    assert!(
        rough > 0.5 * smooth,
        "rough metal goes dark: {rough:.3} of the sky against smooth {smooth:.3}"
    );
    assert!(
        rough < smooth,
        "a rough lobe loses some energy: {rough:.3} vs {smooth:.3}"
    );
}

#[test]
fn a_black_dielectric_reflects_four_percent_head_on() {
    let Some(f0) = sphere("dielectric", black_plastic(0.1)) else {
        return;
    };
    assert!(
        (0.025..0.06).contains(&f0),
        "dielectric F0 should be ~0.04: {f0:.4}"
    );
}
