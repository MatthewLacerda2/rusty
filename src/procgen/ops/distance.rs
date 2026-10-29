//! src/procgen/ops/distance.rs — mask → distance from its edge (#1492).
//!
//! The way Substance rounds any mask: each inside pixel (red > 0.5) reads how far it
//! is from the nearest outside pixel, so a Voronoi cell, a thresholded noise or a
//! stamp becomes a bevel profile for [`super::vector::bump_to_normal`].
//!
//! The distance is **exact** and costs `O(n)` per row: the separable squared
//! Euclidean distance transform of Felzenszwalb & Huttenlocher (*Distance Transforms
//! of Sampled Functions*, 2012), one pass down the columns (as rows of the transposed
//! image) and one along the rows. The domain is **periodic** so the result tiles:
//! each 1-D pass sees the row three times over, `[-n, 2n)`, and since no point of a
//! wrapped row is more than `n / 2` away from another, every wrapped neighbour is in
//! reach.

use super::super::image_buf::Image;

/// The distance from each inside pixel (red > 0.5) to the mask's edge, over
/// `radius` tile units at `resolution`, clamped to `[0, 1]`, as grayscale. The edge
/// sits half a pixel short of the nearest outside pixel's centre, so a pixel next
/// to the outside reads half a pixel. Outside reads 0; a mask with no outside reads
/// 1 everywhere; a `radius` of 0 or less reads 1 everywhere inside.
pub fn distance(img: &Image, radius: f32) -> Image {
    let res = img.resolution();
    let mut seed = Image::new(res);
    seed.for_each_row(|y, row| {
        for (o, p) in row.iter_mut().zip(img.row(y)) {
            o[0] = if p[0] > 0.5 { f32::INFINITY } else { 0.0 };
        }
    });
    let cols = transform_rows(&seed.transposed()).transposed();
    let mut out = transform_rows(&cols);
    let scale = radius * res as f32;
    out.map_in_place(|p| {
        let d = p[0].sqrt();
        let v = if d == 0.0 {
            0.0
        } else if scale > 0.0 {
            ((d - 0.5) / scale).clamp(0.0, 1.0)
        } else {
            1.0
        };
        [v, v, v, 1.0]
    });
    out
}

/// The 1-D periodic transform of every row's red: `min over x' of (x − x')² + f(x')`.
fn transform_rows(src: &Image) -> Image {
    let mut out = Image::new(src.resolution());
    out.for_each_row(|y, row| {
        let f: Vec<f32> = src.row(y).iter().map(|p| p[0]).collect();
        for (o, d) in row.iter_mut().zip(periodic_edt(&f)) {
            o[0] = d;
        }
    });
    out
}

/// The squared distance transform of `f` on a periodic line: `out[x] = min over x'
/// and every wrap k of (x − x' − kn)² + f(x')`. `f` holds 0 (a site) or a squared
/// distance from an earlier pass, and `inf` where no site reaches. It is the lower
/// envelope of the parabolas rooted at `[-n, 2n)`, built and read in one sweep each.
fn periodic_edt(f: &[f32]) -> Vec<f32> {
    let n = f.len() as i64;
    let at = |q: i64| f64::from(f[q.rem_euclid(n) as usize]);
    // Root `q` of each envelope parabola, and where its stretch starts.
    let (mut roots, mut starts): (Vec<i64>, Vec<f64>) = (Vec::new(), Vec::new());
    for q in (-n..2 * n).filter(|&q| at(q).is_finite()) {
        let lifted = at(q) + (q * q) as f64;
        while let (Some(&p), Some(&start)) = (roots.last(), starts.last()) {
            let s = (lifted - at(p) - (p * p) as f64) / (2 * (q - p)) as f64;
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
        return vec![f32::INFINITY; f.len()];
    }
    let mut k = 0;
    (0..n)
        .map(|x| {
            while k + 1 < starts.len() && starts[k + 1] < x as f64 {
                k += 1;
            }
            let dx = (x - roots[k]) as f64;
            (dx * dx + at(roots[k])) as f32
        })
        .collect()
}

#[cfg(test)]
#[path = "distance_tests.rs"]
mod tests;
