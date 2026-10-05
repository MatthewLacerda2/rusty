//! A Box's lightmap cells pinned exactly (#820): each face owns a padded cell of a
//! 3×2 grid, so a bleed-free border surrounds every face in the atlas.

use super::primitives::generate_box;

const PAD: f32 = 0.06;

/// The lightmap-UV bounding rectangle of `face` (four vertices per face).
fn cell(face: usize) -> ([f32; 2], [f32; 2]) {
    let (vertices, _) = generate_box(2.0, 1.0, 3.0);
    let quad = &vertices[face * 4..face * 4 + 4];
    let axis = |a: usize, pick: fn(f32, f32) -> f32, from: f32| {
        quad.iter().map(|v| v.lightmap_uv[a]).fold(from, pick)
    };
    (
        [axis(0, f32::min, f32::MAX), axis(1, f32::min, f32::MAX)],
        [axis(0, f32::max, f32::MIN), axis(1, f32::max, f32::MIN)],
    )
}

fn near(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-6 && (a[1] - b[1]).abs() < 1e-6
}

#[test]
fn the_first_face_fills_the_padded_bottom_left_cell() {
    let (min, max) = cell(0);
    assert!(near(min, [PAD / 3.0, PAD / 2.0]), "{min:?}");
    assert!(near(max, [(1.0 - PAD) / 3.0, (1.0 - PAD) / 2.0]), "{max:?}");
}

#[test]
fn the_fifth_face_starts_one_padded_cell_over_and_up() {
    let (min, _) = cell(4);
    assert!(near(min, [(1.0 + PAD) / 3.0, (1.0 + PAD) / 2.0]), "{min:?}");
}
