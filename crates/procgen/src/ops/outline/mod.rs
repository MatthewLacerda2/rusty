//! crates/procgen/src/ops/outline/ — the vector generators' math (#2190): outlines
//! in tile units filled into a coverage mask ([`fill`]), from SVG path data
//! ([`svg`]; [`stroke`] draws its outline by distance instead) or a font's shaped glyphs ([`text`]).

pub mod fill;
pub mod stroke;
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
