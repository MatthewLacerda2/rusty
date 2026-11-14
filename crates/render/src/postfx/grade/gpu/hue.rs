//! Hue through the tonemap on lavapipe (#1701): a saturated orange, the colour of
//! the warm lamps the issue's game is lit by, ramped from dim to many times display
//! white, keeps its hue through the default (Neutral) tonemapper, where the
//! per-channel ACES fit skews it to yellow.

use super::{chain, Chain, W};
use rusty_components::Tonemap;
use rusty_core::color::srgb_to_linear;
use rusty_scene::authoring::default_post_processing;

const ORANGE: [f32; 3] = [1.0, 0.45, 0.1];

/// How far the default tonemapper may turn the hue, in degrees. The LUT's
/// interpolation and the 8-bit output round; Neutral itself turns it by none.
const BOUND: f32 = 3.0;

/// The HSV hue of `c`, in degrees, for a red-to-yellow colour (red the largest).
fn hue(c: [f32; 3]) -> f32 {
    let lo = c[1].min(c[2]);
    60.0 * (c[1] - c[2]) / (c[0] - lo)
}

/// The scene brightness the ramp's pixel `i` is scaled to (× the orange).
fn scale(i: usize) -> f32 {
    0.02 * 2000f32.powf(i as f32 / (W - 1) as f32)
}

/// The hue turn of each pixel of the ramp under `tonemap`, `None` where the pixel
/// is no longer coloured enough (16 levels between the largest and smallest
/// channel) to have a hue.
fn turns(c: &mut Chain, tonemap: Tonemap) -> Vec<Option<f32>> {
    let row: Vec<[f32; 3]> = (0..W as usize)
        .map(|i| ORANGE.map(|v| v * scale(i)))
        .collect();
    let mut vc = default_post_processing();
    vc.tonemapping.mode = tonemap;
    let out = c.run(&row, 0.0, Some(&vc));
    let want = hue(ORANGE);
    out.iter()
        .map(|p| {
            let coloured = p[0] >= p[2].saturating_add(16);
            coloured.then(|| (hue([0, 1, 2].map(|k| srgb_to_linear(p[k]))) - want).abs())
        })
        .collect()
}

/// The worst turn over the ramp, and the brightness at which the turn first passes
/// [`BOUND`] (`None`: never). The worst turn tops out at 36.7°, orange's distance
/// from pure yellow (60°): a per-channel curve clips red, then green, and what is
/// left is yellow. So two modes at 36.7° both reach yellow; where they pass the
/// bound is what tells them apart.
fn measure(c: &mut Chain, tonemap: Tonemap) -> (f32, Option<f32>) {
    let t = turns(c, tonemap);
    let worst = t.iter().flatten().copied().fold(0.0, f32::max);
    let breaks = t
        .iter()
        .position(|v| v.is_some_and(|v| v > BOUND))
        .map(scale);
    (worst, breaks)
}

#[test]
fn gpu_a_saturated_orange_ramp_keeps_its_hue_through_the_default_tonemapper() {
    let Some(mut c) = chain() else { return };
    assert_eq!(default_post_processing().tonemapping.mode, Tonemap::Neutral);
    let all = Tonemap::ALL.map(|t| (t, measure(&mut c, t)));
    eprintln!("[hue] (worst turn in degrees, brightness it first passes {BOUND}°): {all:?}");
    let of = |t: Tonemap| {
        all.iter()
            .find(|(m, _)| *m == t)
            .map(|p| p.1)
            .expect("listed")
    };
    let (neutral, _) = of(Tonemap::Neutral);
    assert!(neutral <= BOUND, "Neutral turned orange by {neutral}°");
    // The test can tell: the per-channel fit turns the same ramp well past it.
    assert!(of(Tonemap::Aces).0 > 4.0 * BOUND, "{all:?}");
}
