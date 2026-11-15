//! The outline fill (#2190): closed outlines in tile units → an anti-aliased
//! coverage mask, each pixel the exact share of its area the outlines cover.
//!
//! Each edge adds its signed area to an accumulation row per pixel row it crosses
//! (the scanline method `font-rs` and stb_truetype use), and a prefix sum along
//! the row turns those into the winding a pixel reads, fractional where an edge
//! crosses it. The rule maps winding to coverage: `nonzero` takes its magnitude up
//! to 1, `evenodd` folds it, so a hole the second contour cuts reads 0. Where two
//! edges cross inside one pixel the share is the usual analytic approximation.
//!
//! Outlines live in tile units (`u` right, `v` down), so one outline bakes the same
//! shape at any resolution and on a `size` recipe. A [`Shape`] (one or more closed
//! contours: a glyph, a subpath) that crosses the tile's edge wraps onto the
//! torus, as every map tiles: each copy whose box meets the tile is drawn.

use std::collections::BTreeSet;

use glam::Vec2;

use crate::catalog::generators::outline::path::FillRule;
use crate::image_buf::Size;

/// One edge, tile units, `v` down.
pub type Edge = [Vec2; 2];

/// A set of edges whose winding closes: every contour in it ends where it starts.
pub type Shape = Vec<Edge>;

/// The coverage of `shapes` under `rule`, one value in `[0, 1]` per pixel,
/// row-major at `size`.
///
/// A wrapped copy is the same outline again, not another contour of it, so the
/// rule folds each copy's winding on its own and each pixel then takes the most
/// any copy gives it: under `evenodd` a shape wider than the tile never cancels
/// against its own copy, and its edge reads the same where two copies overlap.
/// Copies of a shape narrower than the tile never share a pixel, so the most is
/// exact there. Under `nonzero` folding first or last reads the same, so every
/// copy goes into one pass.
pub fn coverage(shapes: &[Shape], size: Size, rule: FillRule) -> Vec<f32> {
    let (w, h) = (size.width as usize, size.height as usize);
    let mut acc = Accumulator::new(w, h);
    let mut out = vec![0.0; w * h];
    let offsets: BTreeSet<(i64, i64)> = match rule {
        FillRule::Nonzero => BTreeSet::new(),
        FillRule::Evenodd => shapes.iter().flat_map(offsets).collect(),
    };
    if offsets.is_empty() {
        for shape in shapes {
            acc.copies(shape, |_| true);
        }
        acc.resolve_into(rule, &mut out);
        return out;
    }
    for &k in &offsets {
        for shape in shapes {
            acc.copies(shape, |at| at == k);
        }
        acc.resolve_into(rule, &mut out);
        acc.cells.fill(0.0);
    }
    out
}

/// Every whole-tile offset `(kx, ky)` at which a copy of `shape` meets the tile.
fn offsets(shape: &Shape) -> Vec<(i64, i64)> {
    let Some((lo, hi)) = bounds(shape) else {
        return Vec::new();
    };
    copies(lo.y, hi.y)
        .flat_map(|ky| copies(lo.x, hi.x).map(move |kx| (kx, ky)))
        .collect()
}

/// The box of `shape`'s finite points; `None` when it has none.
fn bounds(shape: &Shape) -> Option<(Vec2, Vec2)> {
    let mut points = shape.iter().flatten().filter(|p| p.is_finite());
    let first = *points.next()?;
    Some(points.fold((first, first), |(lo, hi), p| (lo.min(*p), hi.max(*p))))
}

/// The whole-tile offsets `k` for which `[lo + k, hi + k]` meets `(0, 1)`.
fn copies(lo: f32, hi: f32) -> std::ops::RangeInclusive<i64> {
    let first = (-hi).floor() as i64 + 1;
    let last = (1.0 - lo).ceil() as i64 - 1;
    first..=last
}

/// The signed area each edge leaves per pixel, two spare columns per row for an
/// edge on the right border.
struct Accumulator {
    w: usize,
    h: usize,
    cells: Vec<f32>,
}

impl Accumulator {
    /// Add each copy of `shape` (tile units) whose offset `keep` takes.
    fn copies(&mut self, shape: &Shape, keep: impl Fn((i64, i64)) -> bool) {
        let scale = Vec2::new(self.w as f32, self.h as f32);
        for (kx, ky) in offsets(shape).into_iter().filter(|&k| keep(k)) {
            let k = Vec2::new(kx as f32, ky as f32);
            for &[a, b] in shape {
                self.edge((a + k) * scale, (b + k) * scale);
            }
        }
    }

    fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            cells: vec![0.0; (w + 2) * h],
        }
    }

    /// Add the edge `p → q`, in pixels. It is clipped to the rows, and split where
    /// it leaves the columns: a stretch left of the image is an edge on its left
    /// border (it still winds every pixel right of it), one right of it adds nothing.
    fn edge(&mut self, p: Vec2, q: Vec2) {
        if !(p.is_finite() && q.is_finite()) || p.y == q.y {
            return;
        }
        let (dir, a, b) = if p.y < q.y { (1.0, p, q) } else { (-1.0, q, p) };
        let h = self.h as f32;
        if b.y <= 0.0 || a.y >= h {
            return;
        }
        let at_y = |y: f32| a.lerp(b, (y - a.y) / (b.y - a.y));
        let a = if a.y < 0.0 { at_y(0.0) } else { a };
        let b = if b.y > h { at_y(h) } else { b };
        let w = self.w as f32;
        let mut cuts = [0.0, 1.0, 1.0, 1.0];
        for (i, x) in [0.0, w].into_iter().enumerate() {
            let t = (x - a.x) / (b.x - a.x);
            if t > 0.0 && t < 1.0 {
                cuts[i + 1] = t;
            }
        }
        cuts[1..3].sort_by(f32::total_cmp);
        let clamp = |v: Vec2| Vec2::new(v.x.clamp(0.0, w), v.y);
        for pair in cuts.windows(2) {
            if pair[1] > pair[0] {
                let (s, e) = (clamp(a.lerp(b, pair[0])), clamp(a.lerp(b, pair[1])));
                self.inside(s, e, dir);
            }
        }
    }

    /// An edge inside the image, `a` above `b`, winding `dir`.
    fn inside(&mut self, a: Vec2, b: Vec2, dir: f32) {
        let dxdy = (b.x - a.x) / (b.y - a.y);
        let stride = self.w + 2;
        let first = a.y.floor().max(0.0) as usize;
        let last = (b.y.ceil() as usize).min(self.h);
        for row in first..last {
            let top = (row as f32).max(a.y);
            let bottom = ((row + 1) as f32).min(b.y);
            let dy = bottom - top;
            if dy <= 0.0 {
                continue;
            }
            let x_top = a.x + (top - a.y) * dxdy;
            let x_bottom = a.x + (bottom - a.y) * dxdy;
            let line = &mut self.cells[row * stride..(row + 1) * stride];
            span(line, x_top, x_bottom, dy * dir);
        }
    }

    /// Each pixel's coverage, the running sum of its row's cells under `rule`, into
    /// `out` where it is more than `out` holds.
    fn resolve_into(&self, rule: FillRule, out: &mut [f32]) {
        let stride = self.w + 2;
        let rows = self.cells.chunks(stride.max(1)).take(self.h);
        for (row, px) in rows.zip(out.chunks_mut(self.w.max(1))) {
            let mut winding = 0.0;
            for (cell, p) in row[..self.w].iter().zip(px) {
                winding += cell;
                *p = p.max(fold(winding, rule));
            }
        }
    }
}

/// A winding as coverage under `rule`.
fn fold(winding: f32, rule: FillRule) -> f32 {
    let w = winding.abs();
    match rule {
        FillRule::Nonzero => w.min(1.0),
        FillRule::Evenodd => {
            let w = w % 2.0;
            if w > 1.0 {
                2.0 - w
            } else {
                w
            }
        }
    }
}

/// Spread one row's stretch of an edge, from `x0` to `x1` and `d` tall (signed by
/// winding), over `line`: each cell gets the share of `d` the edge leaves to the
/// left of the pixel's right side, less the share already given to its left.
///
/// Both ends are clamped into the row first: an end recomputed from a piece the
/// caller clipped to `x = 0` can land a rounding error below it, and a `lo` a hair
/// under 0 would floor to −1 and spread the whole stretch one cell off.
fn span(line: &mut [f32], x0: f32, x1: f32, d: f32) {
    let right = line.len().saturating_sub(2) as f32;
    let (x0, x1) = (x0.clamp(0.0, right), x1.clamp(0.0, right));
    let (lo, hi) = if x0 < x1 { (x0, x1) } else { (x1, x0) };
    let lo_floor = lo.floor();
    let i = lo_floor as usize;
    let j = hi.ceil() as usize;
    if j <= i + 1 {
        // Within one column: the trapezoid right of the edge's mean x.
        let mid = 0.5 * (x0 + x1) - lo_floor;
        line[i] += d * (1.0 - mid);
        line[i + 1] += d * mid;
        return;
    }
    let s = 1.0 / (hi - lo);
    let lo_frac = lo - lo_floor;
    let first = 0.5 * s * (1.0 - lo_frac) * (1.0 - lo_frac);
    let hi_frac = hi - hi.ceil() + 1.0;
    let end = 0.5 * s * hi_frac * hi_frac;
    line[i] += d * first;
    if j == i + 2 {
        line[i + 1] += d * (1.0 - first - end);
    } else {
        let second = s * (1.5 - lo_frac);
        line[i + 1] += d * (second - first);
        for cell in &mut line[i + 2..j - 1] {
            *cell += d * s;
        }
        let before_end = second + (j - i - 3) as f32 * s;
        line[j - 1] += d * (1.0 - before_end - end);
    }
    line[j] += d * end;
}

#[cfg(test)]
#[path = "fill_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "fill_wrap_tests.rs"]
mod wrap_tests;
