use std::f32::consts::{FRAC_PI_2, TAU};

use super::{finite, gray, Sampler};
use crate::catalog::generators::shape::ShapeProfile;
use crate::ops::field::Field;
use crate::recipe::{Shape, ShapeKind};

/// Past it a polygon is a circle to about a pixel at 2048², and the cap keeps a huge count cheap.
pub const MAX_CORNERS: u32 = 64;

/// Rings of neighbour tiles' copies unioned: a shape wider than 9 tiles keeps a seam.
const MAX_SPILL: i32 = 4;

#[derive(Clone, Copy)]
struct Sdf {
    kind: ShapeKind,
    half: [f32; 2],
    round: f32,
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

pub fn sampler<'a>(p: &Shape, softness: Field<'a>) -> Sampler<'a> {
    let (sdf, profile) = (Sdf::new(p), p.profile);
    let depth = -sdf.at(0.0, 0.0);
    // An odd polygon or star is not symmetric top to bottom, so one past the tile is the
    // union of its neighbours' copies, as a stamp under `tile` is.
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
    fn inward(self, x: f32, y: f32, d: f32, depth: f32) -> f32 {
        // An ellipse's distance is first-order inside, so `d / depth` would plateau along its
        // major axis: its stretched radius gives one peak instead.
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

fn coverage(d: f32, softness: f32) -> f32 {
    if softness > 0.0 {
        (-d / softness).clamp(0.0, 1.0)
    } else if d <= 0.0 {
        1.0
    } else {
        0.0
    }
}

fn height(profile: ShapeProfile, t: f32) -> f32 {
    let out = 1.0 - t;
    match profile {
        ShapeProfile::Flat | ShapeProfile::Cone => t,
        ShapeProfile::Paraboloid => 1.0 - out * out,
        ShapeProfile::Hemisphere => (1.0 - out * out).sqrt(),
    }
}

fn rect(p: [f32; 2], b: [f32; 2], r: f32) -> f32 {
    let q = [p[0] - b[0] + r, p[1] - b[1] + r];
    let outside = q[0].max(0.0).hypot(q[1].max(0.0));
    outside + q[0].max(q[1]).min(0.0) - r
}

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

fn line(p: [f32; 2], b: [f32; 2]) -> f32 {
    let along = (p[0] - (b[0] - b[1]).max(0.0)).max(0.0);
    along.hypot(p[1]) - b[1]
}

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
