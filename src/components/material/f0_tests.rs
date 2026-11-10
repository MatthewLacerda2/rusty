//! A dielectric's F0 from `ior` and `KHR_materials_specular` (#1998).

use super::{dielectric_f0, dielectric_f90};
use crate::components::MaterialAsset;

#[test]
fn the_default_material_reflects_exactly_the_four_percent_it_always_did() {
    // The constant the shader held before #1998, bit for bit.
    assert_eq!(MaterialAsset::default().dielectric_f0(), [0.04_f32; 3]);
    assert_eq!(dielectric_f0(1.5, 1.0, [1.0; 3]), [0.04; 3]);
}

#[test]
fn water_reflects_about_two_percent() {
    let [r, g, b] = dielectric_f0(1.33, 1.0, [1.0; 3]);
    assert!((r - 0.02006).abs() < 1e-4, "{r}");
    assert_eq!((r, r), (g, b));
}

#[test]
fn a_pane_doubles_by_its_colour_and_the_colour_and_strength_shape_it() {
    assert_eq!(dielectric_f0(1.5, 1.0, [2.0; 3]), [0.08; 3]);
    assert_eq!(dielectric_f0(1.5, 0.5, [1.0, 0.5, 0.0]), [0.02, 0.01, 0.0]);
    // The `min` caps a channel at a mirror before the strength scales it.
    assert_eq!(dielectric_f0(1.5, 0.5, [100.0; 3]), [0.5; 3]);
}

#[test]
fn a_malformed_material_still_gives_a_reflectance_in_range() {
    assert_eq!(dielectric_f0(1.0, 1.0, [1.0; 3]), [0.0; 3], "no interface");
    assert_eq!(dielectric_f0(f32::NAN, 1.0, [1.0; 3]), [0.0; 3]);
    assert_eq!(dielectric_f0(-1.0, 1.0, [1.0; 3]), [0.0; 3], "a pole at -1");
    assert_eq!(dielectric_f0(f32::INFINITY, 1.0, [1.0; 3]), [1.0; 3]);
    assert_eq!(dielectric_f0(1.5, f32::NAN, [1.0; 3]), [0.0; 3]);
    assert_eq!(dielectric_f0(1.5, 7.0, [1.0; 3]), [0.04; 3]);
    let odd = dielectric_f0(1.0, 1.0, [f32::INFINITY, f32::NAN, -1.0]);
    assert_eq!(odd, [0.0; 3], "no ior, no reflection, whatever the colour");
    assert_eq!(dielectric_f0(1.5, 1.0, [f32::INFINITY; 3]), [1.0; 3]);
}

#[test]
fn the_grazing_reflectance_is_the_specular_strength_clamped() {
    assert_eq!(MaterialAsset::default().dielectric_f90(), 1.0);
    assert_eq!(dielectric_f90(0.5), 0.5);
    assert_eq!(dielectric_f90(0.0), 0.0, "no rim at specular 0");
    assert_eq!((dielectric_f90(7.0), dielectric_f90(-1.0)), (1.0, 0.0));
    assert_eq!(dielectric_f90(f32::NAN), 0.0, "as dielectric_f0 reads it");
}
