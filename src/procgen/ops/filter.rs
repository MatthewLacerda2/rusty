//! src/procgen/ops/filter.rs — neighbourhood ops.
//!
//! A **separable blur**: a box averages a `(2r+1)` window along each row, then along
//! each column; a Gaussian (the default, #1492) is three box passes per axis whose
//! widths match the variance of `σ = r / 3`, so it reaches about `r` (≈ 10 px for
//! `r = 12`) for three passes' cost; under about 2.5 px it is one exact 3-tap pass
//! instead, since three boxes can't go that narrow. Each pass slides a running sum along the row (#1490), so the blur costs
//! `O(n)` whatever the radius, and the window wraps at the edges so the blurred result
//! stays tiling — its main use is softening a height field before
//! [`super::vector::bump_to_normal`] and feathering masks. The column pass runs as a
//! row pass on the transposed image, so both pass by rows and band in parallel.
//! Each axis takes its own radius in pixels, so on a non-square image the blur
//! reaches the same fraction of the tile both ways (#1493).
//!
//! [`cavity`] and [`curvature`] derive weathering masks from a height field (its red
//! channel) the way Substance does (#408): both wrap, and both take their reach in
//! tile units, so the masks tile and look the same at any resolution.

use super::super::image_buf::{Image, Rgba, Size};
use super::super::recipe::BlurKind;
use super::field::Field;

mod varying;

pub use varying::blur_varying;

/// A blur radius given as a fraction of the tile, in pixels along an axis of
/// `pixels`: `radius * pixels`, at most half the tile (a wider window just wraps
/// back over pixels it already averaged). Negative or NaN reads as no blur.
pub fn pixel_radius(radius: f32, pixels: u32) -> f32 {
    let px = radius * pixels as f32;
    if px > 0.0 {
        px.min((pixels / 2) as f32)
    } else {
        0.0
    }
}

/// [`pixel_radius`] on each axis of `size`: `(across, down)`.
pub fn pixel_radii(radius: f32, size: Size) -> (f32, f32) {
    (
        pixel_radius(radius, size.width),
        pixel_radius(radius, size.height),
    )
}

/// Separable blur reaching `across` pixels along rows and `down` along columns
/// (see [`pixel_radii`] for the recipe's tile-fraction unit). A blur with no pass
/// returns the input unchanged.
pub fn blur(img: &Image, (across, down): (f32, f32), kind: BlurKind) -> Image {
    let (across, down) = (passes(across, kind), passes(down, kind));
    if across.is_empty() && down.is_empty() {
        return img.clone();
    }
    let rows = along_rows(img, &across);
    along_rows(&rows.transposed(), &down).transposed()
}

/// `passes` run in turn along every row of `img`. Each pass replaces the last, so at
/// most four images live at once.
fn along_rows(img: &Image, passes: &[Pass]) -> Image {
    match passes.split_first() {
        None => img.clone(),
        Some((&first, rest)) => rest
            .iter()
            .fold(blur_rows(img, first), |acc, &p| blur_rows(&acc, p)),
    }
}

/// One 1-D pass along a row.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Pass {
    /// A flat, wrapped `(2r+1)` window.
    Box(i64),
    /// The 3-tap kernel `[a, 1 − 2a, a]`, whose variance is `2a`.
    Taps(f32),
}

/// A Gaussian narrower than this variance (px², `r ≈ 2.45 px`) is one [`Pass::Taps`]:
/// three boxes can't go below a radius-1 box's `2/3`, which is where taps meet it.
const TAPS_UP_TO: f32 = 2.0 / 3.0;

/// The passes one axis runs for a blur reaching `radius` pixels, no-op passes
/// dropped. A box is one pass of `round(radius)`. A Gaussian matches the variance of
/// `σ = radius / 3`, so it reaches about `radius` (3σ): three boxes of the odd widths
/// `w` and `w + 2` whose mix is nearest it (Kovesi, *Fast Almost-Gaussian
/// Filtering*, 2010), or under [`TAPS_UP_TO`] one exact 3-tap pass. Matching the
/// variance, not splitting a rounded radius, keeps the spread the same at any
/// resolution, down to a pixel or two (#1492).
fn passes(radius: f32, kind: BlurKind) -> Vec<Pass> {
    let var = (radius / 3.0).powi(2);
    match kind {
        BlurKind::Box => box_passes([radius.round() as i64]),
        BlurKind::Gaussian if radius.is_nan() || radius <= 0.0 => Vec::new(),
        BlurKind::Gaussian if var < TAPS_UP_TO => vec![Pass::Taps(var / 2.0)],
        BlurKind::Gaussian => {
            let var = 12.0 * var;
            let ideal = (var / 3.0 + 1.0).sqrt().floor() as i64;
            let w = ideal - 1 + ideal % 2;
            let wf = w as f32;
            let narrow = (var - 3.0 * wf * wf - 12.0 * wf - 9.0) / (-4.0 * wf - 4.0);
            let narrow = narrow.round().clamp(0.0, 3.0) as usize;
            box_passes((0..3).map(|i| if i < narrow { (w - 1) / 2 } else { (w + 1) / 2 }))
        }
    }
}

/// A box pass per radius, radius 0 (a one-pixel window) dropped.
fn box_passes(radii: impl IntoIterator<Item = i64>) -> Vec<Pass> {
    radii
        .into_iter()
        .filter(|&r| r > 0)
        .map(Pass::Box)
        .collect()
}

/// Run `pass` over every row of `src`, wrapping at the ends.
fn blur_rows(src: &Image, pass: Pass) -> Image {
    let mut out = Image::new(src.size());
    match pass {
        Pass::Box(r) => {
            let inv = 1.0 / (2 * r + 1) as f64;
            out.for_each_row(|y, row| slide(src.row(y), r, inv, row));
        }
        Pass::Taps(a) => out.for_each_row(|y, row| taps(src.row(y), a, row)),
    }
    out
}

/// One row through the wrapped kernel `[a, 1 − 2a, a]`.
fn taps(src: &[Rgba], a: f32, out: &mut [Rgba]) {
    let n = src.len();
    for (x, o) in out.iter_mut().enumerate() {
        let (l, c, r) = (src[(x + n - 1) % n], src[x], src[(x + 1) % n]);
        *o = std::array::from_fn(|i| a * (l[i] + r[i]) + (1.0 - 2.0 * a) * c[i]);
    }
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
        let large = leave.iter().any(|v| v.is_nan() || v.abs() > LARGE);
        if large || acc.iter().any(|a| !a.is_finite()) {
            acc = window(x + 1);
        }
    }
}

/// Cavity mask: how far each pixel sits below its neighbourhood (a Gaussian blur
/// reaching `radius` pixels, `(across, down)`), `1 − max(blur(h) − h, 0)`, as
/// grayscale. Flat and raised ground stay white; crevices go dark — multiply it
/// into albedo for baked occlusion, or invert it for a dirt mask.
pub fn cavity(img: &Image, radius: (f32, f32)) -> Image {
    let around = blur(img, radius, BlurKind::Gaussian);
    let mut out = Image::new(img.size());
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
pub fn curvature(img: &Image, radius: f32, strength: Field) -> Image {
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
    Image::fill_uv(img.size(), |u, v| {
        let mean = ring.iter().map(|(du, dv)| h(u + du, v + dv)).sum::<f32>() / 8.0;
        let c = 0.5 + strength.at_pixel(u, v) * (h(u, v) - mean);
        [c, c, c, 1.0]
    })
}

#[cfg(test)]
#[path = "filter_tests.rs"]
mod tests;
