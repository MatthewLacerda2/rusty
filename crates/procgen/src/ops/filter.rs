use super::super::image_buf::{Image, Rgba, Size};
use super::super::recipe::BlurKind;
use super::field::Field;

mod varying;

pub use varying::blur_varying;

/// `radius * pixels`, at most half the tile (a wider window just wraps). Negative or NaN reads as
/// no blur.
pub fn pixel_radius(radius: f32, pixels: u32) -> f32 {
    let px = radius * pixels as f32;
    if px > 0.0 {
        px.min((pixels / 2) as f32)
    } else {
        0.0
    }
}

pub fn pixel_radii(radius: f32, size: Size) -> (f32, f32) {
    (
        pixel_radius(radius, size.width),
        pixel_radius(radius, size.height),
    )
}

pub fn blur(img: &Image, (across, down): (f32, f32), kind: BlurKind) -> Image {
    let (across, down) = (passes(across, kind), passes(down, kind));
    if across.is_empty() && down.is_empty() {
        return img.clone();
    }
    let rows = along_rows(img, &across);
    along_rows(&rows.transposed(), &down).transposed()
}

fn along_rows(img: &Image, passes: &[Pass]) -> Image {
    match passes.split_first() {
        None => img.clone(),
        Some((&first, rest)) => rest
            .iter()
            .fold(blur_rows(img, first), |acc, &p| blur_rows(&acc, p)),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Pass {
    Box(i64),
    Taps(f32),
}

/// A Gaussian narrower than this variance (px², `r ≈ 2.45 px`) is one [`Pass::Taps`]:
/// three boxes can't go below a radius-1 box's `2/3`, which is where taps meet it.
const TAPS_UP_TO: f32 = 2.0 / 3.0;

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

fn box_passes(radii: impl IntoIterator<Item = i64>) -> Vec<Pass> {
    radii
        .into_iter()
        .filter(|&r| r > 0)
        .map(Pass::Box)
        .collect()
}

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

fn taps(src: &[Rgba], a: f32, out: &mut [Rgba]) {
    let n = src.len();
    for (x, o) in out.iter_mut().enumerate() {
        let (l, c, r) = (src[(x + n - 1) % n], src[x], src[(x + 1) % n]);
        *o = std::array::from_fn(|i| a * (l[i] + r[i]) + (1.0 - 2.0 * a) * c[i]);
    }
}

/// A dropped pixel past this magnitude re-sums the window, so its rounding residue does not
/// linger in the sum. Texture values sit near `[0, 1]`, so a real recipe never meets it.
const LARGE: f32 = 1e3;

/// The sum is `f64`, so a long row's adds and drops do not drift.
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
