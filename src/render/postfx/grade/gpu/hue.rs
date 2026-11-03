//! Hue through the tonemap on lavapipe (#1701): a saturated orange, the colour of
//! the warm lamps the issue's game is lit by, ramped from dim to many times display
//! white, keeps its hue through the default (Neutral) tonemapper, where the
//! per-channel ACES fit skews it to yellow.

use super::{chain, Chain, W};
use crate::components::Tonemap;
use crate::core::color::srgb_to_linear;
use crate::scene::authoring::default_post_processing;

const ORANGE: [f32; 3] = [1.0, 0.45, 0.1];

/// How far the default tonemapper may turn the hue, in degrees. The LUT's
/// interpolation and the 8-bit output round; Neutral itself turns it by none.
const BOUND: f32 = 3.0;

/// The HSV hue of `c`, in degrees, for a red-to-yellow colour (red the largest).
fn hue(c: [f32; 3]) -> f32 {
    let lo = c[1].min(c[2]);
    60.0 * (c[1] - c[2]) / (c[0] - lo)
}

/// The worst hue turn over the ramp under `tonemap`, among pixels still coloured
/// enough (16 levels between the largest and smallest channel) to have a hue.
fn worst_turn(c: &mut Chain, tonemap: Tonemap) -> f32 {
    let scale = |i: u32| 0.02 * 2000f32.powf(i as f32 / (W - 1) as f32);
    let row: Vec<[f32; 3]> = (0..W).map(|i| ORANGE.map(|v| v * scale(i))).collect();
    let mut vc = default_post_processing();
    vc.tonemapping.mode = tonemap;
    let out = c.run(&row, 0.0, Some(&vc));
    let want = hue(ORANGE);
    out.iter()
        .filter(|p| p[0] >= p[2].saturating_add(16))
        .map(|p| (hue([0, 1, 2].map(|k| srgb_to_linear(p[k]))) - want).abs())
        .fold(0.0, f32::max)
}

#[test]
fn gpu_a_saturated_orange_ramp_keeps_its_hue_through_the_default_tonemapper() {
    let Some(mut c) = chain() else { return };
    assert_eq!(default_post_processing().tonemapping.mode, Tonemap::Neutral);
    let turns = Tonemap::ALL.map(|t| (t, worst_turn(&mut c, t)));
    eprintln!("[hue] worst turn in degrees: {turns:?}");
    let turn = |t: Tonemap| turns.iter().find(|(m, _)| *m == t).map_or(0.0, |p| p.1);
    assert!(
        turn(Tonemap::Neutral) <= BOUND,
        "Neutral turned orange by {}°",
        turn(Tonemap::Neutral)
    );
    // The test can tell: the per-channel fit turns the same ramp well past it.
    assert!(turn(Tonemap::Aces) > 4.0 * BOUND, "{turns:?}");
}
