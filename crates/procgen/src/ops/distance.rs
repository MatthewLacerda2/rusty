//! Exact: the separable squared Euclidean distance transform (Felzenszwalb & Huttenlocher, 2012).
//! Periodic so the result tiles: each 1-D pass sees the row at `[-n, 2n)`, as no wrapped point is
//! over `n / 2` away. Non-square: distance is in tile units, so each pass weighs squared offsets by
//! its axis' step; the unit is a pixel of the longer side.

use super::super::image_buf::Image;
use super::field::Field;

/// The edge sits half a pixel short of the nearest outside pixel's centre.
pub fn distance(img: &Image, radius: Field) -> Image {
    let size = img.size();
    let unit = size.longer() as f64;
    let (sx, sy) = (unit / size.width as f64, unit / size.height as f64);
    let mut seed = Image::new(size);
    seed.for_each_row(|y, row| {
        for (o, p) in row.iter_mut().zip(img.row(y)) {
            o[0] = if p[0] > 0.5 { f32::INFINITY } else { 0.0 };
        }
    });
    let cols = transform_rows(&seed.transposed(), sy * sy).transposed();
    let mut out = transform_rows(&cols, sx * sx);
    out.map_indexed(|i, p| {
        let scale = radius.at(i) * unit as f32;
        let d = p[0].sqrt();
        let (ox, oy) = (f64::from(p[1]) * sx, f64::from(p[2]) * sy);
        let step = if ox.abs() >= oy.abs() { sx } else { sy };
        let half = (0.5 * step) as f32;
        let v = if d == 0.0 {
            0.0
        } else if scale > 0.0 {
            ((d - half) / scale).clamp(0.0, 1.0)
        } else {
            1.0
        };
        [v, v, v, 1.0]
    });
    out
}

fn transform_rows(src: &Image, a: f64) -> Image {
    let mut out = Image::new(src.size());
    out.for_each_row(|y, row| {
        let src = src.row(y);
        let f: Vec<f32> = src.iter().map(|p| p[0]).collect();
        let n = f.len() as i64;
        for (x, (o, (d, root))) in row.iter_mut().zip(periodic_edt(&f, a)).enumerate() {
            o[0] = d;
            o[1] = (x as i64 - root) as f32;
            o[2] = src[root.rem_euclid(n) as usize][1];
        }
    });
    out
}

/// `a` is exactly 1 on a square image, where every product by it is exact.
fn periodic_edt(f: &[f32], a: f64) -> Vec<(f32, i64)> {
    let n = f.len() as i64;
    let at = |q: i64| f64::from(f[q.rem_euclid(n) as usize]);
    let (mut roots, mut starts): (Vec<i64>, Vec<f64>) = (Vec::new(), Vec::new());
    for q in (-n..2 * n).filter(|&q| at(q).is_finite()) {
        let lifted = at(q) + a * (q * q) as f64;
        while let (Some(&p), Some(&start)) = (roots.last(), starts.last()) {
            let s = (lifted - at(p) - a * (p * p) as f64) / (a * (2 * (q - p)) as f64);
            if s > start {
                roots.push(q);
                starts.push(s);
                break;
            }
            roots.pop();
            starts.pop();
        }
        if roots.is_empty() {
            roots.push(q);
            starts.push(f64::NEG_INFINITY);
        }
    }
    if roots.is_empty() {
        return vec![(f32::INFINITY, 0); f.len()];
    }
    let mut k = 0;
    (0..n)
        .map(|x| {
            while k + 1 < starts.len() && starts[k + 1] < x as f64 {
                k += 1;
            }
            let dx = (x - roots[k]) as f64;
            ((a * (dx * dx) + at(roots[k])) as f32, roots[k])
        })
        .collect()
}

#[cfg(test)]
#[path = "distance_tests.rs"]
mod tests;
