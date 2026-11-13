//! The unwrap's edges the #831 mutation run left open (#891): one triangle is enough
//! to unwrap, and charts grow by neighbour, not by seed.

use glam::Vec3;

use super::super::{unwrap, LightmapUvSettings};
use super::{assert_valid, chart_rects, on};

#[test]
fn a_single_triangle_unwraps() {
    let positions = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
    let u = unwrap(&positions, &[0, 1, 2], &on()).expect("one triangle unwraps");
    assert_eq!(u.uvs.len(), 3);
    assert_valid(&positions, &u);
}

/// A strip of `quads` unit quads, each bent `bend` degrees up from the last.
fn curved_strip(quads: u32, bend: f32) -> (Vec<[f32; 3]>, Vec<u32>) {
    let mut row = Vec3::ZERO;
    let mut positions = vec![row.to_array(), (row + Vec3::X).to_array()];
    for i in 0..quads {
        let a = (bend * i as f32).to_radians();
        row += Vec3::new(0.0, a.sin(), a.cos());
        positions.extend([row.to_array(), (row + Vec3::X).to_array()]);
    }
    let indices = (0..quads)
        .flat_map(|i| {
            let (a, b, c, d) = (2 * i, 2 * i + 1, 2 * i + 3, 2 * i + 2);
            [a, d, c, a, c, b]
        })
        .collect();
    (positions, indices)
}

#[test]
fn charts_compare_each_face_with_its_neighbour_not_the_seed() {
    // 8° between neighbours, 32° end to end: inside the 45° cone, and every edge is
    // softer than a 10° hard angle, so the whole strip is one chart.
    let (positions, indices) = curved_strip(5, 8.0);
    let settings = LightmapUvSettings {
        hard_angle: 10.0,
        ..on()
    };
    let u = unwrap(&positions, &indices, &settings).unwrap();
    assert_valid(&positions, &u);
    assert_eq!(chart_rects(&u).len(), 1, "one chart, grown face to face");
}
