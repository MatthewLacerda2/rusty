//! src/procgen/ops/filter.rs — neighbourhood ops.
//!
//! A **separable box blur**: average a `(2r+1)` window along each row, then along
//! each column. Each pass slides a running sum along the row (#1490), so the blur
//! costs `O(n)` whatever the radius, and the window wraps at the edges so the
//! blurred result stays tiling — its main use is softening a height field before
//! [`super::vector::bump_to_normal`] and feathering masks. The column pass runs as a
//! row pass on the transposed image, so both pass by rows and band in parallel.
//!
//! [`cavity`] and [`curvature`] derive weathering masks from a height field (its red
//! channel) the way Substance does (#408): both wrap, and both take their reach in
//! tile units, so the masks tile and look the same at any resolution.

use super::super::image_buf::{Image, Rgba};

/// A blur radius given as a fraction of the tile width, in pixels at `resolution`:
/// `round(radius * resolution)`, at most half the tile (a wider window just wraps
/// back over pixels it already averaged). Negative or NaN reads as no blur.
pub fn pixel_radius(radius: f32, resolution: u32) -> u32 {
    let px = (radius * resolution as f32).round();
    if px >= 1.0 {
        (px as u32).min(resolution / 2)
    } else {
        0
    }
}

/// Separable box blur of `radius` **pixels** (see [`pixel_radius`] for the recipe's
/// tile-fraction unit). `radius == 0` returns the input unchanged.
pub fn blur(img: &Image, radius: u32) -> Image {
    if radius == 0 {
        return img.clone();
    }
    // Drop the row pass before the column pass: at most four images live at once.
    let t = blur_rows(img, radius as i64).transposed();
    blur_rows(&t, radius as i64).transposed()
}

/// Box-average every row of `src` over a wrapped `(2r+1)` window.
fn blur_rows(src: &Image, r: i64) -> Image {
    let inv = 1.0 / (2 * r + 1) as f64;
    let mut out = Image::new(src.resolution());
    out.for_each_row(|y, row| slide(src.row(y), r, inv, row));
    out
}

/// A dropped pixel past this magnitude re-sums the window, so its rounding residue
/// doesn't linger in the running sum. Texture values sit near `[0, 1]`, so it never
/// fires on a real recipe; while every pixel stays under it, the residue in an output
/// pixel is below `n × LARGE × ε₆₄` (≈ 5e-10 at a 4096-pixel row).
const LARGE: f32 = 1e3;

/// One row's sliding box average: the window's sum is built once, then each step
/// adds the pixel entering on the right and drops the one leaving on the left. The
/// sum is `f64` so a long row's adds and drops don't drift. A non-finite sum (an
/// `inf` dropped leaves `inf − inf = NaN`) or a dropped [`LARGE`] pixel re-sums the
/// next window instead, so a bad pixel only reaches its own window, as a per-pixel
/// re-sum would.
fn slide(src: &[Rgba], r: i64, inv: f64, out: &mut [Rgba]) {
    let n = src.len() as i64;
    let at = |i: i64| src[i.rem_euclid(n) as usize];
    let window = |centre: i64| {
        let mut acc = [0.0f64; 4];
        for k in centre - r..=centre + r {
            let p = at(k);
            (0..4).for_each(|c| acc[c] += p[c] as f64);
        }
        acc
    };
    let mut acc = window(0);
    for (x, o) in (0..n).zip(out.iter_mut()) {
        *o = acc.map(|a| (a * inv) as f32);
        let (enter, leave) = (at(x + r + 1), at(x - r));
        for c in 0..4 {
            acc[c] += enter[c] as f64 - leave[c] as f64;
        }
        let large = leave.iter().any(|v| !(v.abs() <= LARGE));
        if large || acc.iter().any(|a| !a.is_finite()) {
            acc = window(x + 1);
        }
    }
}

/// Cavity mask: how far each pixel sits below its `radius`-pixel neighbourhood,
/// `1 − max(blur(h) − h, 0)`, as grayscale. Flat and raised ground stay white;
/// crevices go dark — multiply it into albedo for baked occlusion, or invert it for
/// a dirt mask.
pub fn cavity(img: &Image, radius: u32) -> Image {
    let around = blur(img, radius);
    let mut out = Image::new(img.resolution());
    out.for_each_row(|y, row| {
        for ((p, h), b) in row.iter_mut().zip(img.row(y)).zip(around.row(y)) {
            let v = 1.0 - (b[0] - h[0]).max(0.0);
            *p = [v, v, v, 1.0];
        }
    });
    out
}

/// Curvature: `0.5 + strength × (h − ring mean)`, where the ring is eight bilinear,
/// wrapped samples `radius` (a tile fraction) away — a discrete Laplacian at that
/// scale. Above mid-grey is convex (raised edges: wear), below is concave (grime).
/// Bilinear taps keep sub-pixel radii smooth, so it holds across resolutions.
pub fn curvature(img: &Image, radius: f32, strength: f32) -> Image {
    let h = |u: f32, v: f32| img.sample_bilinear_wrapped(u, v)[0];
    let diag = radius * std::f32::consts::FRAC_1_SQRT_2;
    let ring = [
        (radius, 0.0),
        (-radius, 0.0),
        (0.0, radius),
        (0.0, -radius),
        (diag, diag),
        (diag, -diag),
        (-diag, diag),
        (-diag, -diag),
    ];
    let res = img.resolution();
    Image::fill_uv(res, |u, v| {
        let mean = ring.iter().map(|(du, dv)| h(u + du, v + dv)).sum::<f32>() / 8.0;
        let c = 0.5 + strength * (h(u, v) - mean);
        [c, c, c, 1.0]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Today's blur before #1490: every pixel re-sums its whole wrapped window.
    fn naive_blur(img: &Image, r: i64) -> Image {
        let res = img.resolution() as i64;
        let inv = 1.0 / (2 * r + 1) as f32;
        let pass = |src: &Image, horizontal: bool| {
            Image::fill_uv(src.resolution(), |u, v| {
                let (x, y) = ((u * res as f32) as i64, (v * res as f32) as i64);
                let mut acc = [0.0f32; 4];
                for k in -r..=r {
                    let p = match horizontal {
                        true => src.get_wrapped(x + k, y),
                        false => src.get_wrapped(x, y + k),
                    };
                    (0..4).for_each(|c| acc[c] += p[c]);
                }
                acc.map(|a| a * inv)
            })
        };
        pass(&pass(img, true), false)
    }

    #[test]
    fn sliding_blur_matches_the_naive_reference() {
        let img = Image::fill_uv(32, |u, v| {
            let h = (u * 91.7 + v * 13.3).sin() * (v * 47.1).cos();
            [h, u * v, 1.0 - u, h * h]
        });
        // Small, mid, exactly half the tile, and past it (the window wraps twice).
        for r in [1, 3, 8, 16, 40] {
            let (fast, slow) = (blur(&img, r), naive_blur(&img, r as i64));
            for (a, b) in fast.pixels().iter().zip(slow.pixels()) {
                for c in 0..4 {
                    assert!((a[c] - b[c]).abs() < 1e-5, "r={r}: {a:?} vs {b:?}");
                }
            }
        }
    }

    #[test]
    fn a_flat_image_blurs_to_itself_exactly() {
        let img = Image::filled(16, [0.25, 0.5, 0.75, 1.0]);
        assert_eq!(blur(&img, 5), img);
    }

    /// A 32² mid-grey image with one `bad` pixel at (10, 20), blurred at radius 2.
    fn blur_around(bad: f32) -> Image {
        let mut img = Image::filled(32, [0.5; 4]);
        img.pixels_mut()[20 * 32 + 10] = [bad; 4];
        blur(&img, 2)
    }

    /// Whether `(x, y)` lies in the (2r+1)² window around (10, 20), r = 2.
    fn near(x: usize, y: usize) -> bool {
        x.abs_diff(10) <= 2 && y.abs_diff(20) <= 2
    }

    #[test]
    fn an_infinite_pixel_reaches_only_its_window() {
        let out = blur_around(f32::INFINITY);
        for (i, p) in out.pixels().iter().enumerate() {
            let (x, y) = (i % 32, i / 32);
            match near(x, y) {
                true => assert!(p[0].is_infinite(), "({x}, {y}): {p:?}"),
                false => assert!((p[0] - 0.5).abs() < 1e-6, "({x}, {y}): {p:?}"),
            }
        }
    }

    #[test]
    fn a_huge_pixel_leaves_no_residue_once_dropped() {
        let out = blur_around(1e20);
        for (i, p) in out.pixels().iter().enumerate() {
            let (x, y) = (i % 32, i / 32);
            if !near(x, y) {
                assert!((p[0] - 0.5).abs() < 1e-6, "({x}, {y}): {p:?}");
            }
        }
    }
}
