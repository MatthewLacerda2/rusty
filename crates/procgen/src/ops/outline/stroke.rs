//! Coverage is `clamp(r − d + ½)` in pixels from the distance to the polyline; overlapping parts
//! take the nearest segment, never a sum. Distances are in pixels; on a non-square bake the width
//! is measured along a pixel of the geometric mean of its sides.

use glam::Vec2;

use super::svg::Subpath;
use crate::image_buf::Size;

// A quadtree node at most this many pixels wide, or holding this few segments, is measured
// pixel by pixel.
const LEAF_SIDE: f32 = 8.0;
const LEAF_SEGMENTS: usize = 4;

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
    // Each node keeps only the segments whose least distance to it can beat the best
    // greatest distance; an explicit stack, so a path from disk never recurses.
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

fn paint(out: &mut [f32], w: usize, lo: Vec2, hi: Vec2, value: impl Fn(Vec2) -> f32) {
    let (x0, x1) = (lo.x as usize, (hi.x as usize).min(w));
    let rows = out.len() / w.max(1);
    for y in lo.y as usize..(hi.y as usize).min(rows) {
        for x in x0..x1 {
            out[y * w + x] = value(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
        }
    }
}

fn segments(subpaths: &[Subpath], scale: Vec2, r: f32) -> Vec<[Vec2; 2]> {
    let mut out = Vec::new();
    for s in subpaths {
        let mut points: Vec<Vec2> = s.points.iter().map(|p| *p * scale).collect();
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

fn copies(lo: f32, hi: f32, side: f32) -> std::ops::RangeInclusive<i64> {
    let first = (-hi / side).floor() as i64 + 1;
    let last = ((side - lo) / side).ceil() as i64 - 1;
    first..=last
}

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
