use super::*;
use crate::procgen::hash;
use crate::procgen::image_buf::Size;
use crate::procgen::ops::filter::{blur, pixel_radii};

/// White noise on `size`, a busy signal every blur visibly changes.
fn noise(size: Size) -> Image {
    Image::fill_uv(size, |u, v| {
        let x = hash::unit2((u * 64.0) as i64, (v * 64.0) as i64, 0, 9);
        [x, 1.0 - x, 0.5 * x, 1.0]
    })
}

/// The largest per-channel difference between `a` and `b`.
fn gap(a: &Image, b: &Image) -> f32 {
    let pairs = a.pixels().iter().zip(b.pixels());
    pairs
        .flat_map(|(p, q)| (0..4).map(move |c| (p[c] - q[c]).abs()))
        .fold(0.0, f32::max)
}

/// A map whose red is `red(u, v)`, as a field over `[0, 1]`.
fn map(size: Size, red: impl Fn(f32, f32) -> f32 + Sync) -> Image {
    Image::fill_uv(size, |u, v| [red(u, v), 0.0, 0.0, 1.0])
}

fn field(img: &Image) -> Field<'_> {
    Field::Map {
        img,
        min: 0.0,
        max: 1.0,
    }
}

#[test]
fn a_uniform_box_radius_matches_the_fixed_box() {
    // The fixed box rounds its radius; a whole-pixel radius is the same window.
    for size in [Size::new(32, 32), Size::new(64, 32)] {
        let (src, r) = (noise(size), map(size, |_, _| 0.0625));
        let fixed = blur(&src, pixel_radii(0.0625, size), BlurKind::Box);
        let out = blur_varying(&src, field(&r), BlurKind::Box);
        assert!(gap(&out, &fixed) < 1e-5, "{size:?}: {}", gap(&out, &fixed));
    }
}

#[test]
fn a_uniform_gaussian_radius_is_close_to_the_fixed_gaussian() {
    let size = Size::new(64, 32);
    let (src, r) = (noise(size), map(size, |_, _| 0.1));
    let fixed = blur(&src, pixel_radii(0.1, size), BlurKind::Gaussian);
    let out = blur_varying(&src, field(&r), BlurKind::Gaussian);
    assert!(gap(&out, &fixed) < 0.03, "{}", gap(&out, &fixed));
}

#[test]
fn a_zero_radius_keeps_its_pixels_sharp_beside_a_soft_half() {
    let size = Size::new(64, 64);
    let src = noise(size);
    // Radius 0 on the left half, 0.1 on the right.
    let r = map(size, |u, _| if u < 0.5 { 0.0 } else { 0.1 });
    let out = blur_varying(&src, field(&r), BlurKind::Gaussian);
    let w = 64;
    let side = |left: bool| {
        let cols = if left { 0..32 } else { 32..64 };
        let mut d = 0.0f32;
        for y in 0..64 {
            for x in cols.clone() {
                let i = y * w + x;
                d = d.max((out.pixels()[i][0] - src.pixels()[i][0]).abs());
            }
        }
        d
    };
    assert!(side(true) < 1e-5, "the sharp half changed: {}", side(true));
    assert!(side(false) > 0.1, "the soft half did not blur");
}

#[test]
fn the_window_wraps_across_both_edges() {
    let size = Size::new(16, 16);
    let mut src = Image::filled(size, [0.0, 0.0, 0.0, 1.0]);
    src.pixels_mut()[0] = [1.0; 4];
    let r = map(size, |_, _| 0.125);
    let out = blur_varying(&src, field(&r), BlurKind::Box);
    assert!(
        out.get_wrapped(-1, 0)[0] > 0.0,
        "nothing crossed the left edge"
    );
    assert!(
        out.get_wrapped(0, -1)[0] > 0.0,
        "nothing crossed the top edge"
    );
    let total: f32 = out.pixels().iter().map(|p| p[0]).sum();
    assert!(
        (total - 1.0).abs() < 1e-5,
        "the blur lost or made light: {total}"
    );
}

#[test]
fn a_non_finite_pixel_only_reaches_its_own_windows() {
    let size = Size::new(32, 4);
    let mut src = Image::filled(size, [0.5, 0.5, 0.5, 1.0]);
    src.pixels_mut()[3] = [f32::INFINITY; 4];
    let r = map(size, |u, _| if u < 0.5 { 0.03 } else { 0.06 });
    let out = blur_varying(&src, field(&r), BlurKind::Box);
    // Rows away from the bad one, and pixels of its row out of reach, stay 0.5.
    assert!((out.get_wrapped(20, 0)[0] - 0.5).abs() < 1e-6);
    assert!(!out.get_wrapped(3, 0)[0].is_finite());
}
