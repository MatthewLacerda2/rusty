//! The outline fill: exact area, both fill rules, sub-pixel shares and the wrap.

use glam::Vec2;

use super::*;

/// The axis-aligned rectangle from `lo` to `hi`, wound by `ccw`.
fn rect(lo: [f32; 2], hi: [f32; 2], ccw: bool) -> Shape {
    let (lo, hi) = (Vec2::from(lo), Vec2::from(hi));
    let mut p = [lo, Vec2::new(hi.x, lo.y), hi, Vec2::new(lo.x, hi.y)];
    if ccw {
        p.reverse();
    }
    (0..4).map(|i| [p[i], p[(i + 1) % 4]]).collect()
}

fn sum(c: &[f32]) -> f32 {
    c.iter().sum()
}

#[test]
fn a_square_covers_exactly_its_area_at_any_resolution() {
    // Off the pixel grid at every size, so edge pixels are partial.
    let square = rect([0.213, 0.301], [0.713, 0.801], false);
    for side in [7, 16, 33, 128] {
        let size = Size::new(side, side);
        let c = coverage(std::slice::from_ref(&square), size, FillRule::Nonzero);
        let area = sum(&c) / (side * side) as f32;
        assert!((area - 0.25).abs() < 1e-4, "{side}: {area}");
        assert!(c.iter().all(|v| (0.0..=1.0).contains(v)));
    }
    let wide = coverage(&[square], Size::new(64, 16), FillRule::Nonzero);
    assert!(
        (sum(&wide) / 1024.0 - 0.25).abs() < 1e-4,
        "a non-square extent"
    );
}

#[test]
fn a_hole_is_cut_by_evenodd_and_by_an_opposite_winding_under_nonzero() {
    let outer = rect([0.125, 0.125], [0.875, 0.875], false);
    let size = Size::new(32, 32);
    let at = |c: &[f32], x: usize, y: usize| c[y * 32 + x];
    for (inner_ccw, rule, hole) in [
        (false, FillRule::Evenodd, true),
        (true, FillRule::Evenodd, true),
        (false, FillRule::Nonzero, false),
        (true, FillRule::Nonzero, true),
    ] {
        let inner = rect([0.375, 0.375], [0.625, 0.625], inner_ccw);
        let c = coverage(&[outer.clone(), inner], size, rule);
        assert_eq!(
            at(&c, 16, 16),
            if hole { 0.0 } else { 1.0 },
            "{rule:?} {inner_ccw}"
        );
        assert_eq!(at(&c, 6, 6), 1.0, "the ring is filled");
        assert_eq!(at(&c, 1, 1), 0.0, "outside is empty");
    }
}

#[test]
fn a_sliver_reads_its_covered_share() {
    // A tenth of a pixel wide, a whole column tall.
    let c = coverage(
        &[rect([0.5, 0.0], [0.5 + 0.1 / 8.0, 1.0], false)],
        Size::new(8, 8),
        FillRule::Nonzero,
    );
    for y in 0..8 {
        assert!((c[y * 8 + 4] - 0.1).abs() < 1e-5, "{}", c[y * 8 + 4]);
        assert_eq!(c[y * 8 + 3], 0.0);
        assert_eq!(c[y * 8 + 5], 0.0);
    }
}

#[test]
fn a_diagonal_edge_splits_its_pixels_exactly() {
    // The triangle under the diagonal covers half the tile, half of each pixel on it.
    let t = vec![
        [Vec2::new(0.0, 0.0), Vec2::new(1.0, 1.0)],
        [Vec2::new(1.0, 1.0), Vec2::new(0.0, 1.0)],
        [Vec2::new(0.0, 1.0), Vec2::new(0.0, 0.0)],
    ];
    let c = coverage(&[t], Size::new(10, 10), FillRule::Nonzero);
    assert!((sum(&c) - 50.0).abs() < 1e-3);
    for i in 0..10 {
        assert!((c[i * 10 + i] - 0.5).abs() < 1e-5);
    }
}

#[test]
fn a_shape_across_the_edge_wraps_onto_the_other_side() {
    let size = Size::new(16, 16);
    let across = coverage(
        &[rect([0.75, 0.75], [1.25, 1.25], false)],
        size,
        FillRule::Nonzero,
    );
    let inside = coverage(
        &[rect([0.25, 0.25], [0.75, 0.75], false)],
        size,
        FillRule::Nonzero,
    );
    assert!((sum(&across) - sum(&inside)).abs() < 1e-3);
    assert_eq!(
        across[0], 1.0,
        "the corner past the edge lands at the origin"
    );
    assert_eq!(across[15 * 16 + 15], 1.0);
    assert_eq!(across[8 * 16 + 8], 0.0);
}

#[test]
fn an_open_or_degenerate_outline_is_harmless() {
    let size = Size::new(4, 4);
    let nan = vec![[Vec2::new(f32::NAN, 0.0), Vec2::new(0.5, 0.5)]];
    let flat = vec![[Vec2::new(0.0, 0.5), Vec2::new(1.0, 0.5)]];
    for shape in [nan, flat, Vec::new()] {
        let c = coverage(&[shape], size, FillRule::Nonzero);
        assert!(c.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
    }
}
