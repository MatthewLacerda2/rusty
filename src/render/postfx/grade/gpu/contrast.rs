//! Contrast on lavapipe (#1479): URP's, in log space around mid-grey before the
//! tonemap (ACEScc under ACES), so a mild boost darkens dark metal without clipping
//! it to black.

use super::knobs::analytic;
use super::{chain, f16_value, W};
use crate::components::{PostProcessingComponent, Tonemap};
use crate::core::color::linear_to_srgb;
use crate::scene::authoring::default_post_processing;
use crate::scene::authoring::rendering::post_processing as ops;

/// The default (ACES) volume at `contrast`.
fn at(contrast: f32) -> PostProcessingComponent {
    let mut vc = default_post_processing();
    ops::grading::color_adjustments::set_contrast(&mut vc, contrast);
    vc
}

#[test]
fn gpu_contrast_steepens_around_mid_grey_without_clipping_the_shadows() {
    let Some(mut c) = chain() else { return };
    // Pixel 0 black, 1 the issue's dark metal (0.05), 2 a grey card, 3 a highlight.
    let mut row = vec![[0.0f32; 3]; W as usize];
    (row[1], row[2], row[3]) = ([0.05; 3], [0.18; 3], [2.0; 3]);
    let neutral = c.run(&row, 0.0, Some(&default_post_processing()));
    let punchy = c.run(&row, 0.0, Some(&at(1.12)));
    let flat = c.run(&row, 0.0, Some(&at(0.5)));
    let g = |out: &[[u8; 4]], i: usize| out[i][1];
    let head = |out: &[[u8; 4]]| out[..4].iter().map(|p| p[1]).collect::<Vec<_>>();
    let (n, p, f) = (head(&neutral), head(&punchy), head(&flat));
    eprintln!("[contrast] black, 0.05, 0.18, 2.0: 1 {n:?}, 1.12 {p:?}, 0.5 {f:?}");

    assert!(g(&punchy, 1) > 0, "1.12 keeps 0.05 above black");
    assert!(g(&punchy, 1) < g(&neutral, 1), "1.12 darkens the shadows");
    assert!(g(&punchy, 3) > g(&neutral, 3), "1.12 lifts the highlights");
    for out in [&punchy, &flat] {
        let pivot = i16::from(g(out, 2)) - i16::from(g(&neutral, 2));
        assert!(pivot.abs() <= 1, "the 0.18 grey card stays put: {pivot}");
    }
    assert_eq!((g(&neutral, 0), g(&punchy, 0)), (0, 0), "black stays black");
    assert!(g(&flat, 1) > g(&neutral, 1), "0.5 lifts the shadows");
    assert!(
        g(&flat, 3) < g(&neutral, 3),
        "0.5 pulls the highlights down"
    );
    // URP lifts black too; the linear-0.5 pivot lifted it to ~137.
    assert!(
        g(&flat, 0) < 70,
        "0.5 does not wash black out: {}",
        g(&flat, 0)
    );
}

/// Scene-linear inputs on (or a half-float step from) the LUT's texels, from black
/// to past display white, each written as the chain will store it.
fn texel_inputs() -> Vec<f32> {
    // `postfx/grade.wgsl`'s `decode`, at texel i of 0..31.
    let decode = |i: u32| ((i as f32 / 31.0 * 11.668338).exp2() - 1.0) / 64.0;
    (0..W).map(|i| f16_value(decode(i.min(31)))).collect()
}

/// Contrast 1 against the analytic tonemap, which never runs contrast. The LUT and
/// the sRGB encode round, so a texel sits up to a level off; contrast's log round
/// trip moves none of them, so this holds whether the early return is there or not.
/// A curve that isn't the identity at 1 (a toe or pivot that doesn't invert) is
/// several levels off.
#[test]
fn gpu_contrast_one_is_the_analytic_tonemap_under_either_log_space() {
    let Some(mut c) = chain() else { return };
    let xs = texel_inputs();
    let row: Vec<[f32; 3]> = xs.iter().map(|&x| [x; 3]).collect();
    for tonemap in [Tonemap::Aces, Tonemap::Reinhard] {
        let mut vc = at(1.3);
        // Bake a contrast first, so the neutral one below is a rebake, not a cache hit.
        c.run(&row, 0.0, Some(&vc));
        vc.tonemapping.mode = tonemap;
        ops::grading::color_adjustments::set_contrast(&mut vc, 1.0);
        let out = c.run(&row, 0.0, Some(&vc));
        let worst = xs
            .iter()
            .zip(&out)
            .map(|(&x, px)| {
                let want = 255.0 * linear_to_srgb(analytic(x, 0.0, tonemap));
                (want - f32::from(px[1])).abs()
            })
            .fold(0.0, f32::max);
        assert!(
            worst <= 1.0,
            "{tonemap:?}: contrast 1 is {worst} levels off"
        );
    }
}

#[test]
fn gpu_a_contrast_far_out_of_range_bakes_as_two_not_nan() {
    let Some(mut c) = chain() else { return };
    // Black, dark metal, then highlights up to past the table's top texel.
    let mut row = vec![[60.0f32; 3]; W as usize];
    (row[0], row[1], row[2]) = ([0.0; 3], [0.05; 3], [4.0; 3]);
    for tonemap in [Tonemap::Aces, Tonemap::Reinhard] {
        let mut wild = default_post_processing();
        // A scene file writes the field directly, past the op's clamp.
        (wild.tonemapping.mode, wild.color_adjustments.contrast) = (tonemap, 10.0);
        let out = c.run(&row, 0.0, Some(&wild));
        let mut two = wild.clone();
        two.color_adjustments.contrast = 2.0;
        assert_eq!(out, c.run(&row, 0.0, Some(&two)), "{tonemap:?}: held to 2");
        assert!(
            out[2][1] > 200,
            "{tonemap:?}: a highlight is bright, not NaN"
        );
        assert!(
            out[63][1] > 200,
            "{tonemap:?}: the top texel is bright, not NaN"
        );
    }
}
