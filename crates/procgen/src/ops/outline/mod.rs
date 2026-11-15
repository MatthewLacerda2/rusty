//! crates/procgen/src/ops/outline/ — the vector generators' math (#2190): outlines
//! in tile units filled into a coverage mask ([`fill`]), from SVG path data
//! ([`svg`], stroked by [`stroke`]) or a font's shaped glyphs ([`text`]).

pub mod fill;
pub mod svg;
pub mod text;

use glam::Vec2;

use fill::{Edge, Shape};
use svg::Subpath;

/// Each subpath closed into a [`Shape`], as SVG fills an open one.
pub fn filled(subpaths: &[Subpath]) -> Vec<Shape> {
    subpaths.iter().map(|s| closed(&s.points)).collect()
}

/// The edges of the polygon through `points`, closed back to its first.
fn closed(points: &[Vec2]) -> Shape {
    let mut edges: Vec<Edge> = points.windows(2).map(|w| [w[0], w[1]]).collect();
    if let (Some(&first), Some(&last)) = (points.first(), points.last()) {
        if first != last {
            edges.push([last, first]);
        }
    }
    edges
}

/// Most sides a round join or cap is cut into.
const MAX_SIDES: usize = 64;

/// `subpaths`' outlines `width` tiles wide, round-joined and round-capped: a
/// quad along each edge and a disc at each point, every one wound the same way,
/// so their union fills under `nonzero`. A closed subpath strokes its closing edge.
pub fn stroke(subpaths: &[Subpath], width: f32) -> Vec<Shape> {
    let r = width / 2.0;
    let sides = ((std::f32::consts::TAU * r * 1024.0).sqrt().ceil() as usize).clamp(8, MAX_SIDES);
    let mut shapes = Vec::new();
    for s in subpaths {
        let mut points = s.points.clone();
        if s.closed && points.first() != points.last() {
            points.extend(points.first().copied());
        }
        for w in points.windows(2) {
            let along = (w[1] - w[0]).normalize_or_zero();
            if along == Vec2::ZERO {
                continue;
            }
            let n = along.perp() * r;
            shapes.push(wound(&[w[0] + n, w[1] + n, w[1] - n, w[0] - n]));
        }
        for &p in &points {
            let disc: Vec<Vec2> = (0..sides)
                .map(|i| {
                    let t = std::f32::consts::TAU * i as f32 / sides as f32;
                    p + Vec2::new(t.cos(), t.sin()) * r
                })
                .collect();
            shapes.push(wound(&disc));
        }
    }
    shapes
}

/// The polygon through `points` as a [`Shape`] of positive signed area.
fn wound(points: &[Vec2]) -> Shape {
    let area: f32 = closed(points).iter().map(|[a, b]| a.perp_dot(*b)).sum();
    if area < 0.0 {
        let reversed: Vec<Vec2> = points.iter().rev().copied().collect();
        closed(&reversed)
    } else {
        closed(points)
    }
}
