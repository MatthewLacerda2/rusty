//! The outline fill at the tile's edge: a stretch ending a rounding error past
//! x = 0, a shape wrapped across the left edge, and `evenodd` on a shape wider
//! than the tile.

use glam::Vec2;

use super::*;

#[test]
fn a_stretch_ending_a_hair_left_of_zero_spreads_from_column_zero() {
    // An edge clipped at x = 0 can come back as −1e-7: it is the same stretch.
    let (mut hair, mut zero) = (vec![0.0; 6], vec![0.0; 6]);
    span(&mut hair, -1e-7, 0.3, 1.0);
    span(&mut zero, 0.0, 0.3, 1.0);
    for (a, b) in hair.iter().zip(&zero) {
        assert!((a - b).abs() < 1e-5, "{hair:?} against {zero:?}");
    }
    assert!((zero[0] - 0.85).abs() < 1e-6);
}

#[test]
fn a_shape_across_the_left_edge_reads_as_itself_moved() {
    // Slanted edges cut at x = 0 on many rows; moved by whole pixels, the mask
    // only rolls, column 0 included.
    let tri = |dx: f32| {
        let p = [
            Vec2::new(-0.2 + dx, 0.1),
            Vec2::new(0.3 + dx, 0.53),
            Vec2::new(-0.13 + dx, 0.9),
        ];
        (0..3).map(|i| [p[i], p[(i + 1) % 3]]).collect::<Shape>()
    };
    let size = Size::new(64, 64);
    let here = coverage(&[tri(0.0)], size, FillRule::Nonzero);
    let moved = coverage(&[tri(3.0 / 64.0)], size, FillRule::Nonzero);
    for y in 0..64 {
        for x in 0..64 {
            let (a, b) = (here[y * 64 + x], moved[y * 64 + (x + 3) % 64]);
            assert!((a - b).abs() < 1e-3, "({x}, {y}): {a} against {b}");
        }
    }
}

#[test]
fn evenodd_never_cancels_a_shape_against_its_own_wrapped_copy() {
    // A stripe a tile and a half wide overlaps its copy, which must not cut it.
    let p = [(-0.25, 0.4), (1.25, 0.4), (1.25, 0.6), (-0.25, 0.6)].map(Vec2::from);
    let stripe: Shape = (0..4).map(|i| [p[i], p[(i + 1) % 4]]).collect();
    let c = coverage(&[stripe], Size::new(32, 32), FillRule::Evenodd);
    assert!(
        c[16 * 32..17 * 32].iter().all(|v| *v == 1.0),
        "{:?}",
        &c[16 * 32..17 * 32]
    );
    assert!(c[..8 * 32].iter().all(|v| *v == 0.0));
}
