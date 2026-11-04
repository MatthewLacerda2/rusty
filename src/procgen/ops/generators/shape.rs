//! One centred shape per tile (#407, #1740): circle, rect, rounded rect, line,
//! polygon or star, as a mask with an optional inward edge ramp, or as a height
//! stamp. Each shape is a signed distance in tile units. The first four are read at
//! `|u − ½|, |v − ½|` of the wrapped point, so they are symmetric about the centre
//! and meet themselves at the wrap. A polygon or star with an odd count is not
//! symmetric top to bottom, so one that reaches past the tile is the union of its
//! copies in the neighbouring tiles: it tiles as a stamp under `tile` does.

use std::f32::consts::{FRAC_PI_2, TAU};

use super::{finite, gray, Sampler};
use crate::procgen::catalog::generators::shape::ShapeProfile;
use crate::procgen::ops::field::Field;
use crate::procgen::recipe::{Shape, ShapeKind};

/// The most `sides` or `points` a shape takes: past it a polygon is a circle to the
/// pixel at 2048², and the cap keeps a huge count cheap.
pub const MAX_CORNERS: u32 = 64;

/// The most rings of copies a shape larger than the tile unions: one that reaches
/// farther (wider than 9 tiles) keeps a seam where the missing copies would be.
const MAX_SPILL: i32 = 4;

/// The shape's signed distance, built once from its params.
#[derive(Clone, Copy)]
struct Sdf {
    kind: ShapeKind,
    half: [f32; 2],
    round: f32,
    /// A polygon's or star's corner count and inner-vertex ratio.
    corners: (f32, f32),
}

impl Sdf {
    fn new(p: &Shape) -> Self {
        let half = p.size.map(|s| s.abs() * 0.5);
        let n = match p.kind {
            ShapeKind::Star => p.points,
            _ => p.sides,
        };
        let n = n.clamp(3, MAX_CORNERS) as f32;
        let ratio = match p.kind {
            ShapeKind::Star => finite(p.inner_radius, 0.5).clamp(0.0, 1.0),
            _ => (TAU / n * 0.5).cos(),
        };
        Self {
            kind: p.kind,
            half,
            round: p.roundness.clamp(0.0, 1.0) * half[0].min(half[1]),
            corners: (n, ratio),
        }
    }

    /// The distance at `(x, y)` from the tile centre (`y` down).
    fn at(self, x: f32, y: f32) -> f32 {
        let (p, half) = ([x.abs(), y.abs()], self.half);
        match self.kind {
            ShapeKind::Circle => ellipse(p, half),
            ShapeKind::Rect => rect(p, half, 0.0),
            ShapeKind::RoundedRect => rect(p, half, self.round),
            ShapeKind::Line => line(p, half),
            ShapeKind::Polygon | ShapeKind::Star => {
                let (hx, hy) = (half[0].max(1e-6), half[1].max(1e-6));
                star([x / hx, y / hy], self.corners) * hx.min(hy)
            }
        }
    }

    /// How many rings of neighbouring tiles' copies of a polygon or star reach into
    /// this one: 0 while it fits, at most [`MAX_SPILL`].
    fn spill(self) -> i32 {
        let reach = self.half[0].max(self.half[1]) - 0.5;
        match self.kind {
            ShapeKind::Polygon | ShapeKind::Star if reach > 0.0 => {
                (reach.ceil() as i32).min(MAX_SPILL)
            }
            _ => 0,
        }
    }
}

/// The shape sampler. `size` is width/height in tiles; `roundness` (clamped to
/// `[0, 1]`) is a `rounded_rect`'s corner radius as a fraction of its half short
/// side; `softness` > 0 ramps a `flat` mask from 0 at the edge to 1 that far inside,
/// read at each pixel (#1494). Another `profile` is a height from 0 at the edge to 1
/// at the centre, the shape's deepest point.
pub fn sampler<'a>(p: &Shape, softness: Field<'a>) -> Sampler<'a> {
    let (sdf, profile) = (Sdf::new(p), p.profile);
    let depth = -sdf.at(0.0, 0.0);
    let k = sdf.spill();
    Box::new(move |u, v| {
        let (x, y) = (u.rem_euclid(1.0) - 0.5, v.rem_euclid(1.0) - 0.5);
        let d = if k > 0 {
            let copies = (-k..=k).flat_map(|i| (-k..=k).map(move |j| (i as f32, j as f32)));
            copies.fold(f32::MAX, |d, (i, j)| d.min(sdf.at(x + i, y + j)))
        } else {
            sdf.at(x, y)
        };
        gray(match profile {
            ShapeProfile::Flat => coverage(d, softness.at_pixel(u, v)),
            _ => height(profile, sdf.inward(x, y, d, depth)),
        })
    })
}

impl Sdf {
    /// How far `(x, y)`, at distance `d`, is from the edge toward the deepest point,
    /// from 0 at the edge to 1 there, for a shape `depth` deep (0 for a shape of no
    /// area, or a malformed size). An ellipse's is its stretched radius, `1 − k0`, a
    /// dome with one peak at the centre: its distance estimate is only first-order
    /// inside, and `d / depth` would clamp to 1 along the major axis, a plateau.
    fn inward(self, x: f32, y: f32, d: f32, depth: f32) -> f32 {
        let t = if self.kind == ShapeKind::Circle {
            let (rx, ry) = (self.half[0].max(1e-6), self.half[1].max(1e-6));
            1.0 - (x / rx).hypot(y / ry)
        } else if depth > 0.0 && depth.is_finite() {
            -d / depth
        } else {
            0.0
        };
        if t.is_nan() {
            0.0
        } else {
            t.clamp(0.0, 1.0)
        }
    }
}

/// 1 inside, 0 outside; with `softness`, a linear ramp over that depth inside.
fn coverage(d: f32, softness: f32) -> f32 {
    if softness > 0.0 {
        (-d / softness).clamp(0.0, 1.0)
    } else if d <= 0.0 {
        1.0
    } else {
        0.0
    }
}

/// `profile`'s height at `t` of the way from the edge (0) to the deepest point (1).
fn height(profile: ShapeProfile, t: f32) -> f32 {
    let out = 1.0 - t;
    match profile {
        ShapeProfile::Flat | ShapeProfile::Cone => t,
        ShapeProfile::Paraboloid => 1.0 - out * out,
        ShapeProfile::Hemisphere => (1.0 - out * out).sqrt(),
    }
}

/// Signed distance to a box of half-extents `b` whose corners are rounded by `r`.
fn rect(p: [f32; 2], b: [f32; 2], r: f32) -> f32 {
    let q = [p[0] - b[0] + r, p[1] - b[1] + r];
    let outside = q[0].max(0.0).hypot(q[1].max(0.0));
    outside + q[0].max(q[1]).min(0.0) - r
}

/// Signed distance to an ellipse of semi-axes `r` (exact on its boundary and for a
/// circle; a first-order estimate elsewhere on an ellipse).
fn ellipse(p: [f32; 2], r: [f32; 2]) -> f32 {
    let (rx, ry) = (r[0].max(1e-6), r[1].max(1e-6));
    let k0 = (p[0] / rx).hypot(p[1] / ry);
    let k1 = (p[0] / (rx * rx)).hypot(p[1] / (ry * ry));
    if k1 == 0.0 {
        -rx.min(ry)
    } else {
        k0 * (k0 - 1.0) / k1
    }
}

/// Signed distance to a horizontal capsule `2·b[0]` long and `2·b[1]` thick.
fn line(p: [f32; 2], b: [f32; 2]) -> f32 {
    let along = (p[0] - (b[0] - b[1]).max(0.0)).max(0.0);
    along.hypot(p[1]) - b[1]
}

/// Signed distance to a star of `n` points on the unit circle, one straight up
/// (`−y`), its inner vertices at `ratio`. A polygon is the star whose inner vertices
/// sit on its edges' midpoints (`ratio = cos(π/n)`). The point is folded into the
/// half-wedge between a point and the next inner vertex, where the one segment
/// between them is the nearest edge (each mirror line puts the edge's reflection
/// farther away).
fn star(q: [f32; 2], (n, ratio): (f32, f32)) -> f32 {
    let wedge = TAU / n;
    let mut beta = (q[1].atan2(q[0]) + FRAC_PI_2).rem_euclid(wedge);
    if beta > wedge * 0.5 {
        beta = wedge - beta;
    }
    let rho = q[0].hypot(q[1]);
    let p = [rho * beta.cos(), rho * beta.sin()];
    let b = [ratio * (wedge * 0.5).cos(), ratio * (wedge * 0.5).sin()];
    let e = [b[0] - 1.0, b[1]];
    let w = [p[0] - 1.0, p[1]];
    let t = ((w[0] * e[0] + w[1] * e[1]) / (e[0] * e[0] + e[1] * e[1])).clamp(0.0, 1.0);
    let dist = (w[0] - e[0] * t).hypot(w[1] - e[1] * t);
    let inside = e[0] * w[1] - e[1] * w[0] > 0.0;
    if inside {
        -dist
    } else {
        dist
    }
}

#[cfg(test)]
#[path = "shape_tests.rs"]
mod tests;
