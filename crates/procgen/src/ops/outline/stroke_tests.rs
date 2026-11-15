//! The distance stroke: anti-aliased edges, round caps at any resolution, the
//! wrap, and its cost bound.

use glam::Vec2;

use super::*;
use crate::ops::outline::svg;

fn stroke(d: &str, width: f32, side: u32) -> Vec<f32> {
    coverage(&svg::parse(d).unwrap(), width, Size::new(side, side))
}

#[test]
fn an_arcs_edge_reads_its_distance_share_not_a_saturated_sum() {
    // The review's arc: a half circle of radius 0.3 about (0.5, 0.5), 0.05 wide.
    let side = 1024;
    let c = stroke("M.2 .5 A.3 .3 0 1 1 .8 .5", 0.05, side);
    let r = 0.025 * side as f32;
    let centre = Vec2::splat(0.5 * side as f32);
    let mut edge = 0;
    for (i, v) in c.iter().enumerate() {
        let p = Vec2::new((i % 1024) as f32 + 0.5, (i / 1024) as f32 + 0.5);
        if p.y > centre.y - 2.0 {
            continue; // the arc's ends, where its caps sit
        }
        // Distance from the stroke's outer or inner edge, in pixels.
        let off = ((p - centre).length() - 0.3 * side as f32).abs() - r;
        let want = (0.5 - off).clamp(0.0, 1.0);
        assert!((v - want).abs() < 0.02, "{p}: {v} against {want}");
        edge += usize::from(off.abs() < 0.25);
    }
    assert!(edge > 100, "edge pixels read about half: {edge}");
}

#[test]
fn a_cap_is_round_at_every_resolution() {
    for side in [256u32, 2048] {
        let c = stroke("M.3 .5 H.7", 0.2, side);
        let s = side as f32;
        let (end, r) = (Vec2::new(0.7, 0.5) * s, 0.1 * s);
        for deg in (0..=90).step_by(15) {
            let t = (deg as f32).to_radians();
            // Just inside and just outside the cap's circle, along this bearing.
            for (gap, inside) in [(-1.5, true), (1.5, false)] {
                let p = end + Vec2::new(t.cos(), t.sin()) * (r + gap);
                let v = c[p.y as usize * side as usize + p.x as usize];
                assert_eq!(v > 0.5, inside, "{side} at {deg}°: {v}");
            }
        }
    }
}

#[test]
fn a_stroke_wraps_and_a_closed_one_strokes_its_closing_edge() {
    let across = stroke("M.9 .5 H1.1", 0.1, 64);
    assert_eq!(across[32 * 64], 1.0, "the far end lands at u = 0");
    assert_eq!(across[32 * 64 + 63], 1.0);
    let closed = stroke("M.25 .25 H.75 V.75 H.25 Z", 0.05, 64);
    assert_eq!(closed[32 * 64 + 16], 1.0, "the closing edge");
    assert_eq!(closed[32 * 64 + 32], 0.0, "the inside stays open");
    assert_eq!(
        stroke("M.5 .5 L.5 .5", 0.1, 64)[32 * 64 + 32],
        1.0,
        "a zero-length run is a dot"
    );
    assert_eq!(
        stroke("M.5 .5 Z", 0.1, 64)[32 * 64 + 32],
        1.0,
        "so is a closed point"
    );
}

#[test]
fn a_dense_fat_stroke_visits_few_segments_per_pixel() {
    // Sixty one-tile circles at the widest stroke, near the most points a path
    // flattens to: every pixel is covered, and the quadtree fills them whole
    // instead of measuring each against every segment.
    let circles: String = (0..60)
        .map(|i| {
            format!(
                "M{} .5 a.3 .3 0 1 0 .6 0 a.3 .3 0 1 0 -.6 0 ",
                0.2 + i as f32 * 1e-3
            )
        })
        .collect();
    let c = stroke(&circles, 1.0, 256);
    assert!(c.iter().all(|v| *v == 1.0));
}
