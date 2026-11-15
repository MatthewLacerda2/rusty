//! A stroked outline (#2190): each pixel's coverage from its distance to the
//! polyline, `clamp(r − d + ½)` in pixels, so a stroke's edge is anti-aliased the
//! same along a straight run, a curve, a round join and a round cap, with no facets
//! at any resolution. Overlapping parts of one stroke take the nearest segment,
//! never a sum, so they never saturate.
//!
//! A pixel visits only the segments that can be nearest it: the image is split as
//! a quadtree, and each node keeps from its parent's list only the segments whose
//! least distance to the node can beat the best greatest distance any segment has
//! to it. A node that one segment covers wholly is filled, one no segment reaches
//! is left empty, and only a node of a few pixels or a few segments is measured
//! pixel by pixel. The walk keeps an explicit stack: it never recurses.
//!
//! Distances are in pixels, the outline mapped onto the bake as the fill maps it;
//! on a bake that is not square the width is measured along a pixel of the
//! geometric mean of its sides. Like the fill, a stroke wraps: each segment is
//! copied at every whole-tile offset where its reach meets the tile.

use glam::Vec2;

use super::svg::Subpath;
use crate::image_buf::Size;

/// A node this many pixels wide, or holding this few segments, is measured per pixel.
const LEAF_SIDE: f32 = 8.0;
const LEAF_SEGMENTS: usize = 4;

/// The coverage of `subpaths` stroked `width` tiles wide, round-joined and
/// round-capped, one value in `[0, 1]` per pixel, row-major at `size`. A closed
/// subpath strokes its closing edge; a zero-length run is a dot.
pub fn coverage(subpaths: &[Subpath], width: f32, size: Size) -> Vec<f32> {
    let (w, h) = (size.width as usize, size.height as usize);
    let mut out = vec![0.0; w * h];
    let scale = Vec2::new(w as f32, h as f32);
    let r = width / 2.0 * (scale.x * scale.y).sqrt();
    if r.is_nan() || r <= 0.0 || w == 0 || h == 0 {
        return out;
    }
    let segments = segments(subpaths, scale, r);
    let all: Vec<u32> = (0..segments.len() as u32).collect();
    let mut stack = vec![(Vec2::ZERO, scale, all)];
    while let Some((lo, hi, list)) = stack.pop() {
        let (c, half) = ((lo + hi) / 2.0, (hi - lo).length() / 2.0);
        let near = |i: &u32| distance(segments[*i as usize], c);
        let best = list.iter().map(near).fold(f32::INFINITY, f32::min) + half;
        if best <= r - 0.5 {
            paint(&mut out, w, lo, hi, |_| 1.0);
            continue;
        }
        let reach = best.min(r + 0.5 + half);
        let kept: Vec<u32> = list
            .into_iter()
            .filter(|i| near(i) - half <= reach)
            .collect();
        if kept.is_empty() {
            continue;
        }
        let side = hi - lo;
        if kept.len() <= LEAF_SEGMENTS || side.max_element() <= LEAF_SIDE {
            paint(&mut out, w, lo, hi, |p| {
                let mut d = f32::INFINITY;
                for &i in &kept {
                    d = d.min(distance(segments[i as usize], p));
                    if d <= r - 0.5 {
                        break;
                    }
                }
                (r - d + 0.5).clamp(0.0, 1.0)
            });
            continue;
        }
        let mid = ((lo + hi) / 2.0).floor();
        let xs = [(lo.x, mid.x), (mid.x, hi.x)];
        let ys = [(lo.y, mid.y), (mid.y, hi.y)];
        for (x0, x1) in xs.into_iter().filter(|(a, b)| b > a) {
            for (y0, y1) in ys.into_iter().filter(|(a, b)| b > a) {
                stack.push((Vec2::new(x0, y0), Vec2::new(x1, y1), kept.clone()));
            }
        }
    }
    out
}

/// Each pixel whose centre lies in `[lo, hi)`, set to `value` of that centre.
fn paint(out: &mut [f32], w: usize, lo: Vec2, hi: Vec2, value: impl Fn(Vec2) -> f32) {
    let (x0, x1) = (lo.x as usize, (hi.x as usize).min(w));
    let rows = out.len() / w.max(1);
    for y in lo.y as usize..(hi.y as usize).min(rows) {
        for x in x0..x1 {
            out[y * w + x] = value(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
        }
    }
}

/// The polyline's segments in pixels, one per pair of points, each copied at every whole-tile offset where it comes within
/// `r` and a pixel of the image.
fn segments(subpaths: &[Subpath], scale: Vec2, r: f32) -> Vec<[Vec2; 2]> {
    let mut out = Vec::new();
    for s in subpaths {
        let mut points: Vec<Vec2> = s.points.iter().map(|p| *p * scale).collect();
        // Closed back to its start; a closed lone point is a zero-length run, a dot.
        if s.closed && (points.len() == 1 || points.first() != points.last()) {
            points.extend(points.first().copied());
        }
        let pairs = points.windows(2).map(|w| [w[0], w[1]]);
        for [a, b] in pairs.filter(|[a, b]| a.is_finite() && b.is_finite()) {
            let pad = r + 1.0;
            let (lo, hi) = (a.min(b) - pad, a.max(b) + pad);
            for ky in copies(lo.y, hi.y, scale.y) {
                for kx in copies(lo.x, hi.x, scale.x) {
                    let k = Vec2::new(kx as f32, ky as f32) * scale;
                    out.push([a + k, b + k]);
                }
            }
        }
    }
    out
}

/// The whole-tile offsets `k` for which `[lo, hi] + k · side` meets `(0, side)`.
fn copies(lo: f32, hi: f32, side: f32) -> std::ops::RangeInclusive<i64> {
    let first = (-hi / side).floor() as i64 + 1;
    let last = ((side - lo) / side).ceil() as i64 - 1;
    first..=last
}

/// The distance from `p` to the segment `[a, b]`.
fn distance([a, b]: [Vec2; 2], p: Vec2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_squared();
    let t = if len2 > 0.0 {
        ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (a + ab * t).distance(p)
}

#[cfg(test)]
#[path = "stroke_tests.rs"]
mod tests;
