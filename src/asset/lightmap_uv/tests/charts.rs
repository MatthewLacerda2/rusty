//! Charting and the pack margin (#831).

use glam::Vec3;

use super::super::{unwrap, LightmapUvSettings, REFERENCE_TEXELS_PER_UNIT};
use super::{assert_valid, chart_rects, cube, on};

#[test]
fn a_cube_unwraps_into_six_separate_padded_charts() {
    let (positions, indices) = cube(2.0);
    let u = unwrap(&positions, &indices, &on()).unwrap();
    assert_valid(&positions, &u);
    let rects = chart_rects(&u);
    assert_eq!(rects.len(), 6, "a 90° edge is hard at the default 88°");
    // One face is 2 × 2 world units: UV units per world unit, from its rectangle.
    let per_world = (rects[0].1 - rects[0].0).max_element() / 2.0;
    let margin = on().pack_margin / REFERENCE_TEXELS_PER_UNIT * per_world;
    for (i, a) in rects.iter().enumerate() {
        for b in &rects[i + 1..] {
            let gap = (b.0 - a.1).max(a.0 - b.1).max_element();
            assert!(gap >= margin * 0.999, "charts {gap} apart, margin {margin}");
        }
    }
}

#[test]
fn a_softer_hard_angle_keeps_a_bevel_in_one_chart() {
    // Two quads meeting at 30°: one chart at 88°, two at 20°.
    let tilt = Vec3::new(0.0, 30f32.to_radians().sin(), 30f32.to_radians().cos());
    let p = [
        Vec3::ZERO,
        Vec3::X,
        Vec3::X + Vec3::Z,
        Vec3::Z,
        Vec3::Z + tilt,
        Vec3::X + Vec3::Z + tilt,
    ];
    let positions: Vec<[f32; 3]> = p.iter().map(|v| v.to_array()).collect();
    let indices = [0, 3, 2, 0, 2, 1, 3, 4, 5, 3, 5, 2];
    let count = |hard_angle| {
        let s = LightmapUvSettings { hard_angle, ..on() };
        chart_rects(&unwrap(&positions, &indices, &s).unwrap()).len()
    };
    assert_eq!((count(88.0), count(20.0)), (1, 2));
}

#[test]
fn nothing_to_unwrap_is_none() {
    assert_eq!(unwrap(&[[0.0; 3]; 2], &[0, 1], &on()), None);
    assert_eq!(unwrap(&[[0.0; 3]; 3], &[0, 1, 5], &on()), None);
}

#[test]
fn the_margin_is_measured_at_the_bakes_default_resolution() {
    let bake = crate::scene::lighting::lightmap::BakeSettings::default();
    assert_eq!(REFERENCE_TEXELS_PER_UNIT, bake.texels_per_unit);
}
