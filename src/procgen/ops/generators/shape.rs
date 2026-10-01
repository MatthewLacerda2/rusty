//! One centred shape per tile (#407): circle, rect, rounded rect or line, as a mask
//! with an optional inward edge ramp. Each shape is a signed distance in tile units,
//! read at `|u − ½|, |v − ½|` of the wrapped point, so it is symmetric about the
//! centre and meets itself at the wrap.

use super::{gray, Sampler};
use crate::procgen::recipe::ShapeKind;

/// The shape sampler. `size` is width/height in tiles; `roundness` (clamped to
/// `[0, 1]`) is a `rounded_rect`'s corner radius as a fraction of its half short
/// side; `softness` > 0 ramps the mask from 0 at the edge to 1 that far inside.
pub fn sampler(kind: ShapeKind, size: [f32; 2], roundness: f32, softness: f32) -> Sampler {
    let half = size.map(|s| s.abs() * 0.5);
    let round = roundness.clamp(0.0, 1.0) * half[0].min(half[1]);
    Box::new(move |u, v| {
        let p = [
            (u.rem_euclid(1.0) - 0.5).abs(),
            (v.rem_euclid(1.0) - 0.5).abs(),
        ];
        let d = match kind {
            ShapeKind::Circle => ellipse(p, half),
            ShapeKind::Rect => rect(p, half, 0.0),
            ShapeKind::RoundedRect => rect(p, half, round),
            ShapeKind::Line => line(p, half),
        };
        gray(coverage(d, softness))
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn at(kind: ShapeKind, size: [f32; 2], softness: f32, u: f32, v: f32) -> f32 {
        sampler(kind, size, 0.5, softness)(u, v)[0]
    }

    #[test]
    fn each_kind_covers_its_centre_and_not_its_outside() {
        for kind in [
            ShapeKind::Circle,
            ShapeKind::Rect,
            ShapeKind::RoundedRect,
            ShapeKind::Line,
        ] {
            assert_eq!(at(kind, [0.6, 0.2], 0.0, 0.5, 0.5), 1.0, "{kind:?}");
            assert_eq!(at(kind, [0.6, 0.2], 0.0, 0.5, 0.75), 0.0, "{kind:?}");
            assert_eq!(at(kind, [0.6, 0.2], 0.0, 0.85, 0.5), 0.0, "{kind:?}");
        }
    }

    #[test]
    fn corners_tell_rect_rounded_rect_and_circle_apart() {
        // Just inside the corner of a 0.6-square.
        let (u, v) = (0.79, 0.79);
        assert_eq!(at(ShapeKind::Rect, [0.6, 0.6], 0.0, u, v), 1.0);
        assert_eq!(at(ShapeKind::RoundedRect, [0.6, 0.6], 0.0, u, v), 0.0);
        assert_eq!(at(ShapeKind::Circle, [0.6, 0.6], 0.0, u, v), 0.0);
        assert_eq!(at(ShapeKind::Circle, [0.6, 0.6], 0.0, 0.79, 0.5), 1.0);
    }

    #[test]
    fn softness_ramps_inward_from_the_edge() {
        // Circle of radius 0.3: 0.05 inside the edge, with a 0.1 ramp, reads 0.5.
        let mid = at(ShapeKind::Circle, [0.6, 0.6], 0.1, 0.75, 0.5);
        assert!((mid - 0.5).abs() < 1e-4, "{mid}");
        assert_eq!(at(ShapeKind::Circle, [0.6, 0.6], 0.1, 0.5, 0.5), 1.0);
        assert_eq!(at(ShapeKind::Circle, [0.6, 0.6], 0.1, 0.81, 0.5), 0.0);
    }
}
