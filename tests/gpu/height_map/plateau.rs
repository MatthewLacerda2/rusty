//! A pixel on a full-height plateau is where the ray meets the surface: the march hits at its
//! start. Without that test, a plateau's far edge, whose first step lands in the pit beyond,
//! drew the pit there.

use super::{frame, grazing, stepped};

/// Pixels bright in the flat frame (the plateau's top) that the marched frame draws dark.
fn eaten(flat: &image::RgbImage, marched: &image::RgbImage) -> usize {
    let luma = |p: &image::Rgb<u8>| p.0.iter().map(|&c| u32::from(c)).sum::<u32>();
    let bright = flat.pixels().map(luma).max().unwrap_or(0);
    let pairs = flat.pixels().zip(marched.pixels());
    pairs
        .filter(|(f, m)| luma(f) * 4 > bright * 3 && luma(m) * 4 < bright)
        .count()
}

#[test]
fn a_plateaus_far_edge_keeps_its_top() {
    let cam = grazing(15.0);
    let flat = frame(stepped(4.0, None), &cam, "rusty_2300_plateau_flat.png");
    let marched = frame(stepped(4.0, Some(0.1)), &cam, "rusty_2300_plateau_pom.png");
    let (Some(flat), Some(marched)) = (flat, marched) else {
        eprintln!("[height-map] no GPU adapter — skipping");
        return;
    };
    let n = eaten(&flat, &marched);
    eprintln!("[height-map] plateau pixels drawn as pit: {n}");
    // Measured on lavapipe: 256 (two rows of 128) before the march tested its start, 0 after.
    assert_eq!(n, 0, "no plateau top draws the pit beyond its edge");
}
