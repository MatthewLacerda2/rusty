//! Scanline accumulation, as `font-rs` and stb_truetype.
//! Tile units, `u` right, `v` down; a Shape over the tile edge wraps: each copy meeting the tile is drawn.

use glam::Vec2;

use crate::catalog::generators::outline::path::FillRule;
use crate::image_buf::Size;

pub type Edge = [Vec2; 2];

pub type Shape = Vec<Edge>;

/// A wrapped copy is the same outline again, not another contour, so where copies of one
/// shape overlap it winds as the copy that winds most: it never cancels against itself.
pub fn coverage(shapes: &[Shape], size: Size, rule: FillRule) -> Vec<f32> {
    let (w, h) = (size.width as usize, size.height as usize);
    let mut shared = Accumulator::new(w, h);
    let mut large: Option<(Vec<f32>, Vec<f32>, Accumulator)> = None;
    for shape in shapes {
        let at = offsets(shape);
        if !larger_than_tile(shape) {
            for &k in &at {
                shared.copy(shape, k);
            }
            continue;
        }
        let (sum, most, copy) = large
            .get_or_insert_with(|| (vec![0.0; w * h], vec![0.0; w * h], Accumulator::new(w, h)));
        most.fill(0.0);
        for &k in &at {
            copy.cells.fill(0.0);
            copy.copy(shape, k);
            copy.windings(|i, v| {
                if v.abs() > most[i].abs() {
                    most[i] = v;
                }
            });
        }
        for (s, m) in sum.iter_mut().zip(most.iter()) {
            *s += m;
        }
    }
    let mut out = vec![0.0; w * h];
    let sum = large.as_ref().map(|l| &l.0);
    shared.windings(|i, v| out[i] = fold(v + sum.map_or(0.0, |s| s[i]), rule));
    out
}

fn larger_than_tile(shape: &Shape) -> bool {
    bounds(shape).is_some_and(|(lo, hi)| hi.x - lo.x > 1.0 || hi.y - lo.y > 1.0)
}

fn offsets(shape: &Shape) -> Vec<(i64, i64)> {
    let Some((lo, hi)) = bounds(shape) else {
        return Vec::new();
    };
    copies(lo.y, hi.y)
        .flat_map(|ky| copies(lo.x, hi.x).map(move |kx| (kx, ky)))
        .collect()
}

fn bounds(shape: &Shape) -> Option<(Vec2, Vec2)> {
    let mut points = shape.iter().flatten().filter(|p| p.is_finite());
    let first = *points.next()?;
    Some(points.fold((first, first), |(lo, hi), p| (lo.min(*p), hi.max(*p))))
}

fn copies(lo: f32, hi: f32) -> std::ops::RangeInclusive<i64> {
    let first = (-hi).floor() as i64 + 1;
    let last = (1.0 - lo).ceil() as i64 - 1;
    first..=last
}

struct Accumulator {
    w: usize,
    h: usize,
    cells: Vec<f32>,
}

impl Accumulator {
    fn copy(&mut self, shape: &Shape, (kx, ky): (i64, i64)) {
        let scale = Vec2::new(self.w as f32, self.h as f32);
        let k = Vec2::new(kx as f32, ky as f32);
        for &[a, b] in shape {
            self.edge((a + k) * scale, (b + k) * scale);
        }
    }

    fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            cells: vec![0.0; (w + 2) * h],
        }
    }

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

    fn inside(&mut self, a: Vec2, b: Vec2, dir: f32) {
        let dxdy = (b.x - a.x) / (b.y - a.y);
        if !dxdy.is_finite() {
            return;
        }
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

    fn windings(&self, mut f: impl FnMut(usize, f32)) {
        let stride = self.w + 2;
        for (y, row) in self.cells.chunks(stride.max(1)).take(self.h).enumerate() {
            let mut winding = 0.0;
            for (x, cell) in row[..self.w].iter().enumerate() {
                winding += cell;
                f(y * self.w + x, winding);
            }
        }
    }
}

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

/// Both ends are clamped into the row: an end recomputed from a piece clipped to `x = 0`
/// can land a rounding error below it and spread the stretch one cell off.
fn span(line: &mut [f32], x0: f32, x1: f32, d: f32) {
    let right = line.len().saturating_sub(2) as f32;
    let (x0, x1) = (x0.clamp(0.0, right), x1.clamp(0.0, right));
    let (lo, hi) = if x0 < x1 { (x0, x1) } else { (x1, x0) };
    let lo_floor = lo.floor();
    let i = lo_floor as usize;
    let j = hi.ceil() as usize;
    if j <= i + 1 {
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
