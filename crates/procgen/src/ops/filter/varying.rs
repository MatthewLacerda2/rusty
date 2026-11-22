//! A box of radius `r` is one unrounded window `2r + 1` wide: rounding would step visibly at each
//! half pixel, so it matches the fixed box only at whole radii. A window with fractional ends adds
//! more variance than the `(W² − 1) / 12` of a whole odd window, so each width is solved from its
//! real variance ([`gaussian_width`]).

use super::super::super::image_buf::{Image, Rgba};
use super::super::super::recipe::BlurKind;
use super::super::field::Field;
use super::{pixel_radii, pixel_radius, LARGE};

pub fn blur_varying(img: &Image, radius: Field, kind: BlurKind) -> Image {
    if let Some(r) = radius.constant() {
        return super::blur(img, pixel_radii(r, img.size()), kind);
    }
    per_pixel(img, radius, kind)
}

fn per_pixel(img: &Image, radius: Field, kind: BlurKind) -> Image {
    let (w, h) = (img.width(), img.height());
    let n = img.pixels().len();
    let across: Vec<f32> = (0..n)
        .map(|i| width(pixel_radius(radius.at(i), w), kind))
        .collect();
    let down: Vec<f32> = (0..n)
        .map(|t| {
            let (x, y) = (t / h as usize, t % h as usize);
            width(pixel_radius(radius.at(y * w as usize + x), h), kind)
        })
        .collect();
    let rows = passes(img, &across, kind);
    passes(&rows.transposed(), &down, kind).transposed()
}

fn width(radius: f32, kind: BlurKind) -> f32 {
    match kind {
        BlurKind::Box => 2.0 * radius + 1.0,
        BlurKind::Gaussian => {
            let sigma = f64::from(radius) / 3.0;
            gaussian_width(sigma * sigma / 3.0) as f32
        }
    }
}

fn gaussian_width(var: f64) -> f64 {
    if var.is_nan() || var <= 0.0 {
        return 1.0;
    }
    let mut m = ((1.0 + 12.0 * var).sqrt() - 1.0).div_euclid(2.0);
    if m * (m + 1.0) / 3.0 > var {
        m -= 1.0;
    } else if (m + 1.0) * (m + 2.0) / 3.0 <= var {
        m += 1.0;
    }
    let squares = m * (m + 1.0) * (2.0 * m + 1.0) / 3.0;
    let f = (var * (2.0 * m + 1.0) - squares) / (2.0 * ((m + 1.0).powi(2) - var));
    2.0 * m + 1.0 + 2.0 * f.clamp(0.0, 1.0)
}

fn passes(img: &Image, widths: &[f32], kind: BlurKind) -> Image {
    let count = match kind {
        BlurKind::Box => 1,
        BlurKind::Gaussian => 3,
    };
    (1..count).fold(pass(img, widths), |acc, _| pass(&acc, widths))
}

fn pass(src: &Image, widths: &[f32]) -> Image {
    let mut out = Image::new(src.size());
    let w = src.width() as usize;
    out.for_each_row(|y, row| {
        let line = src.row(y);
        let wide = &widths[y * w..(y + 1) * w];
        let sat = Table::of(line);
        for (x, o) in row.iter_mut().enumerate() {
            let half = 0.5 * f64::from(wide[x].clamp(1.0, w as f32));
            let centre = x as f64 + 0.5;
            let (a, b) = (centre - half, centre + half);
            let sum = match &sat {
                Some(t) => t.integral(line, a, b),
                None => direct(line, a, b),
            };
            *o = sum.map(|s| (s / (b - a)) as f32);
        }
    });
    out
}

struct Table {
    p: Vec<[f64; 4]>,
}

impl Table {
    /// The row's table, or `None` when a pixel is non-finite or past [`LARGE`]: a
    /// difference of running sums would carry it into every later window, so such a
    /// row sums each window directly instead.
    fn of(line: &[Rgba]) -> Option<Self> {
        let bad = |v: &f32| !v.is_finite() || v.abs() > LARGE;
        if line.iter().flatten().any(bad) {
            return None;
        }
        let mut p = Vec::with_capacity(line.len() + 1);
        let mut acc = [0.0f64; 4];
        p.push(acc);
        for px in line {
            (0..4).for_each(|c| acc[c] += f64::from(px[c]));
            p.push(acc);
        }
        Some(Self { p })
    }

    fn at(&self, line: &[Rgba], t: f64) -> [f64; 4] {
        let n = line.len() as i64;
        let q = t.floor() as i64;
        let (k, laps) = (q.rem_euclid(n), q.div_euclid(n) as f64);
        let (total, before, px) = (self.p[n as usize], self.p[k as usize], line[k as usize]);
        let frac = t - q as f64;
        std::array::from_fn(|c| laps * total[c] + before[c] + frac * f64::from(px[c]))
    }

    fn integral(&self, line: &[Rgba], a: f64, b: f64) -> [f64; 4] {
        let (hi, lo) = (self.at(line, b), self.at(line, a));
        std::array::from_fn(|c| hi[c] - lo[c])
    }
}

fn direct(line: &[Rgba], a: f64, b: f64) -> [f64; 4] {
    let n = line.len() as i64;
    let mut acc = [0.0f64; 4];
    for k in a.floor() as i64..b.ceil() as i64 {
        let cover = b.min((k + 1) as f64) - a.max(k as f64);
        if cover > 0.0 {
            let px = line[k.rem_euclid(n) as usize];
            (0..4).for_each(|c| acc[c] += cover * f64::from(px[c]));
        }
    }
    acc
}

#[cfg(test)]
#[path = "varying_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "width_tests.rs"]
mod width_tests;
