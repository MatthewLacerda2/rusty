//! The outline fill at the tile's edge: wrap, `evenodd` on shapes wider than the tile, and flat
//! edges.

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
    let p = [(-0.25, 0.4), (1.25, 0.4), (1.25, 0.6), (-0.25, 0.6)].map(Vec2::from);
    let stripe: Shape = (0..4).map(|i| [p[i], p[(i + 1) % 4]]).collect();
    let c = coverage(&[stripe], Size::new(32, 32), FillRule::Evenodd);
    assert!(
        c[16 * 32..17 * 32].iter().all(|v| *v == 1.0),
        "{:?}",
        &c[16 * 32..17 * 32]
    );
    assert!(c[..8 * 32].iter().all(|v| *v == 0.0));
    let p = [(-0.25, 0.41), (1.25, 0.41), (1.25, 0.6), (-0.25, 0.6)].map(Vec2::from);
    let stripe: Shape = (0..4).map(|i| [p[i], p[(i + 1) % 4]]).collect();
    let c = coverage(&[stripe], Size::new(32, 32), FillRule::Evenodd);
    let edge = &c[13 * 32..14 * 32];
    assert!(edge.iter().all(|v| (v - edge[0]).abs() < 1e-5), "{edge:?}");
}

fn poly(p: &[(f32, f32)], d: Vec2) -> Shape {
    let p: Vec<Vec2> = p.iter().map(|&q| Vec2::from(q) + d).collect();
    (0..p.len()).map(|i| [p[i], p[(i + 1) % p.len()]]).collect()
}

#[test]
fn moving_shapes_over_the_seam_only_rolls_the_mask() {
    let shapes: [&[(f32, f32)]; 4] = [
        &[(0.05, 0.4), (0.3, 0.4), (0.3, 0.6), (0.05, 0.6)],
        &[(0.9, 0.45), (1.2, 0.45), (1.2, 0.55), (0.9, 0.55)],
        &[(0.7, -0.2), (1.1, 0.3), (0.6, 0.25)],
        &[(-0.3, 0.7), (1.4, 0.7), (1.4, 0.8), (-0.3, 0.8)],
    ];
    let size = Size::new(40, 40);
    let draw = |moves: [(f32, f32); 4], px: Vec2| {
        let set: Vec<Shape> = shapes
            .iter()
            .zip(moves)
            .map(|(p, m)| poly(p, Vec2::from(m) + px / 40.0))
            .collect();
        coverage(&set, size, FillRule::Evenodd)
    };
    let base = draw([(0.0, 0.0); 4], Vec2::ZERO);
    assert!(base[20 * 40 + 4] < 1e-5, "{}", base[20 * 40 + 4]);
    for moves in [
        [(0.0, 0.0), (-1.0, 0.0), (0.0, 0.0), (0.0, 0.0)],
        [(1.0, 0.0), (0.0, 1.0), (-1.0, 1.0), (2.0, -1.0)],
    ] {
        for shift in [Vec2::ZERO, Vec2::new(3.0, 0.0), Vec2::new(-5.0, 7.0)] {
            let moved = draw(moves, shift);
            let (sx, sy) = (shift.x as i64, shift.y as i64);
            for y in 0..40i64 {
                for x in 0..40i64 {
                    let a = base[(y * 40 + x) as usize];
                    let at = ((y + sy).rem_euclid(40) * 40 + (x + sx).rem_euclid(40)) as usize;
                    let b = moved[at];
                    assert!(
                        (a - b).abs() < 1e-3,
                        "{moves:?} {shift}: ({x}, {y}) {a} against {b}"
                    );
                }
            }
        }
    }
}

#[test]
fn an_edge_too_flat_to_divide_by_winds_nothing() {
    // Its height divides to an infinite slope: no NaN may reach the row's sum,
    // whichever way the edge runs.
    let size = Size::new(256, 256);
    for x in [0.0, 0.5] {
        let sliver = poly(&[(x, 0.0), (0.5 - x, 1e-40), (0.5 - x, 0.5)], Vec2::ZERO);
        let flat = poly(&[(x, 0.0), (0.5 - x, 0.0), (0.5 - x, 0.5)], Vec2::ZERO);
        let (a, b) = (
            coverage(&[sliver], size, FillRule::Nonzero),
            coverage(&[flat], size, FillRule::Nonzero),
        );
        for (i, (a, b)) in a.iter().zip(&b).enumerate() {
            assert!((a - b).abs() < 1e-5, "x = {x}, pixel {i}: {a} against {b}");
        }
    }
}
