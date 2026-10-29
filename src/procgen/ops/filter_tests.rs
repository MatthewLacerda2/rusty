//! The separable blur against a naive reference, its handling of bad pixels, and
//! the Gaussian's shape and pass widths (#1490, #1492).

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
        let (fast, slow) = (
            blur(&img, r as f32, BlurKind::Box),
            naive_blur(&img, r as i64),
        );
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
    for kind in [BlurKind::Box, BlurKind::Gaussian] {
        assert_eq!(blur(&img, 5.0, kind), img);
    }
}

/// A 32² mid-grey image with one `bad` pixel at (10, 20), blurred at radius 2.
fn blur_around(bad: f32) -> Image {
    let mut img = Image::filled(32, [0.5; 4]);
    img.pixels_mut()[20 * 32 + 10] = [bad; 4];
    blur(&img, 2.0, BlurKind::Box)
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

#[test]
fn a_gaussian_spreads_a_bright_pixel_symmetrically_within_its_radius() {
    let mut img = Image::filled(64, [0.0; 4]);
    img.pixels_mut()[32 * 64 + 32] = [1.0; 4];
    let out = blur(&img, 12.0, BlurKind::Gaussian);
    let at = |dx: i64, dy: i64| out.get_wrapped(32 + dx, 32 + dy)[0];
    let peak = at(0, 0);
    // Points at one distance from the centre, on an axis and off it. A box is
    // flat to its square's edge, so (13, 0) reads 0 and (5, 12) its peak.
    for [a, b] in [[(5, 0), (3, 4)], [(10, 0), (6, 8)], [(13, 0), (5, 12)]] {
        let (on, off) = (at(a.0, a.1), at(b.0, b.1));
        assert!((on - off).abs() < 0.05 * peak, "{a:?} {on} vs {b:?} {off}");
    }
    assert_eq!(at(13, 0), 0.0, "reaches no further than its radius");
    // Its spread along an axis is σ² = (12 / 3)² = 16. The image is the 1-D
    // kernel g times itself, so a row through the centre is g(dx)·g(0).
    let row = (-32..32).map(|dx| (dx * dx) as f32 * at(dx, 0));
    let var = row.sum::<f32>() / peak.sqrt();
    assert!((var - 16.0).abs() < 2.0, "σ² = {var}");
    let total: f32 = out.pixels().iter().map(|p| p[0]).sum();
    assert!((total - 1.0).abs() < 1e-4, "keeps the energy: {total}");
}

#[test]
fn a_gaussian_mixes_two_box_widths_and_a_box_rounds() {
    let boxes = |radii: &[i64]| radii.iter().map(|&r| Pass::Box(r)).collect::<Vec<_>>();
    assert_eq!(passes(12.0, BlurKind::Gaussian), boxes(&[3, 3, 4]));
    assert_eq!(passes(4.4, BlurKind::Box), boxes(&[4]));
    assert!(passes(0.4, BlurKind::Box).is_empty());
    assert!(passes(0.0, BlurKind::Gaussian).is_empty());
}

/// Under ≈ 2.45 px three boxes can't match the variance (every width is 1 below
/// ≈ 1.7 px, a no-op), so one 3-tap pass matches it exactly; at the switch it is
/// the radius-1 box Kovesi's widths give there.
#[test]
fn a_narrow_gaussian_is_one_exact_three_tap_pass() {
    assert_eq!(passes(1.5, BlurKind::Gaussian), [Pass::Taps(0.125)]);
    let at_switch = 3.0 * TAPS_UP_TO.sqrt();
    let [Pass::Taps(a)] = passes(at_switch - 1e-3, BlurKind::Gaussian)[..] else {
        panic!("taps just under the switch");
    };
    assert!(
        (a - 1.0 / 3.0).abs() < 1e-3,
        "the radius-1 box's weights: {a}"
    );
    assert_eq!(passes(at_switch + 1e-3, BlurKind::Gaussian), [Pass::Box(1)]);
    let mut img = Image::filled(16, [0.0; 4]);
    img.pixels_mut()[8 * 16 + 8] = [1.0; 4];
    let out = blur(&img, 1.5, BlurKind::Gaussian);
    // A row through the centre is g(dx)·g(0), so its σ² is the kernel's.
    let row: Vec<(f32, f32)> = (-8..8)
        .map(|dx| ((dx * dx) as f32, out.get_wrapped(8 + dx, 8)[0]))
        .collect();
    let mass: f32 = row.iter().map(|r| r.1).sum();
    let var = row.iter().map(|(d2, v)| d2 * v).sum::<f32>() / mass;
    assert!((var - 0.25).abs() < 1e-5, "σ² = (1.5 / 3)²: {var}");
}
