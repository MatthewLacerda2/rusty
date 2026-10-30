//! The window a varying blur picks (#1601): a Gaussian reaching `r` pixels is three
//! passes of a window whose variance is `(r / 3)² / 3`, at every radius, and a blur
//! varying down the image blurs each row by its own radius.
//!
//! Equivalent mutants of `gaussian_width` (cargo-mutants reports them; none changes
//! a width by more than rounding):
//! - the first guess at `m` (`- 1.0` → `+` or `/`, `1.0 + 12.0 * var` → `*`) lands
//!   at most one off, and the correction that follows puts it back;
//! - the downward correction's test (`>` → `==` or `>=`) only differs on a span's
//!   exact lower edge, where `m` and `m − 1` give the same window (`f` = 0 or 1);
//! - `m * (m + 1)` → `m / (m + 1)`, `m * (m - 1)` or `m * (m * 1)` only shrinks the
//!   left side, which never exceeds `var` for a correct `m`, so it never fires.

use super::*;
use crate::procgen::hash;
use crate::procgen::image_buf::Size;

/// The variance (px²) of a window `w` wide: pixels `−m..=m` whole, the two past
/// them weighted by the fraction `f` of `w = 2m + 1 + 2f`.
fn variance(w: f64) -> f64 {
    let m = ((w - 1.0) / 2.0).floor();
    let f = (w - 1.0) / 2.0 - m;
    let squares = m * (m + 1.0) * (2.0 * m + 1.0) / 3.0;
    (squares + 2.0 * f * (m + 1.0).powi(2)) / w
}

#[test]
fn a_gaussian_pass_window_has_a_third_of_the_target_variance_at_every_radius() {
    for i in 1..=600 {
        let r = i as f32 * 0.05; // up to 30 px
        let want = (f64::from(r) / 3.0).powi(2) / 3.0;
        let got = variance(f64::from(width(r, BlurKind::Gaussian)));
        assert!(
            (got - want).abs() < 1e-4 * want.max(1.0),
            "{r} px: {got} vs {want}"
        );
    }
}

#[test]
fn a_variance_a_hair_under_a_whole_window_still_picks_that_window() {
    // `1 + 12 var` rounds to 25 here, which first guesses one span too wide.
    for (edge, whole) in [(2.0f64, 5.0), (20.0 / 3.0, 9.0), (24.0, 17.0)] {
        let under = f64::from_bits(edge.to_bits() - 1);
        let w = gaussian_width(under);
        assert!((w - whole).abs() < 1e-9, "{under}: {w}, not {whole}");
    }
}

#[test]
fn no_variance_is_a_one_pixel_window() {
    for var in [0.0, -1.0, f64::NAN, f64::NEG_INFINITY] {
        assert_eq!(gaussian_width(var), 1.0, "{var}");
    }
}

#[test]
fn a_radius_varying_down_the_image_blurs_each_row_by_its_own() {
    // Stripes that vary only down the image: only the column pass changes them.
    let size = Size::new(32, 64);
    let src = Image::fill_uv(size, |_, v| {
        let x = hash::unit2(0, (v * 64.0) as i64, 0, 3);
        [x, x, x, 1.0]
    });
    let radius = Image::fill_uv(size, |_, v| {
        [if v < 0.5 { 0.0 } else { 0.1 }, 0.0, 0.0, 1.0]
    });
    let field = Field::Map {
        img: &radius,
        min: 0.0,
        max: 1.0,
    };
    let out = blur_varying(&src, field, BlurKind::Box);
    let row_gap = |y: i64| {
        (0..32)
            .map(|x| (out.get_wrapped(x, y)[0] - src.get_wrapped(x, y)[0]).abs())
            .fold(0.0, f32::max)
    };
    assert!(row_gap(8) < 1e-6, "a sharp row changed: {}", row_gap(8));
    assert!(row_gap(48) > 0.05, "a soft row did not blur");
}
