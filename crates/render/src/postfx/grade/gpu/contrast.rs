use super::analytic::analytic;
use super::{chain, f16_value, W};
use rusty_components::{PostProcessingComponent, Tonemap};
use rusty_core::color::linear_to_srgb;
use rusty_scene::create::default_post_processing;

fn at(contrast: f32) -> PostProcessingComponent {
    let mut vc = default_post_processing();
    vc.tonemapping.mode = Tonemap::Aces;
    vc.color_adjustments.contrast = contrast;
    vc
}

#[test]
fn gpu_contrast_steepens_around_mid_grey_without_clipping_the_shadows() {
    let Some(mut c) = chain() else { return };
    let mut row = vec![[0.0f32; 3]; W as usize];
    (row[1], row[2], row[3]) = ([0.05; 3], [0.18; 3], [2.0; 3]);
    let neutral = c.run(&row, 0.0, Some(&at(1.0)));
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
    assert!(
        g(&flat, 0) < 70,
        "0.5 does not wash black out: {}",
        g(&flat, 0)
    );
}

fn texel_inputs() -> Vec<f32> {
    let decode = |i: u32| ((i as f32 / 31.0 * 11.668338).exp2() - 1.0) / 64.0;
    (0..W).map(|i| f16_value(decode(i.min(31)))).collect()
}

#[test]
fn gpu_contrast_one_is_the_analytic_tonemap_under_either_log_space() {
    let Some(mut c) = chain() else { return };
    let xs = texel_inputs();
    let row: Vec<[f32; 3]> = xs.iter().map(|&x| [x; 3]).collect();
    for tonemap in [Tonemap::Aces, Tonemap::Reinhard] {
        let mut vc = at(1.3);
        c.run(&row, 0.0, Some(&vc));
        vc.tonemapping.mode = tonemap;
        vc.color_adjustments.contrast = 1.0;
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
    let mut row = vec![[60.0f32; 3]; W as usize];
    (row[0], row[1], row[2]) = ([0.0; 3], [0.05; 3], [4.0; 3]);
    for tonemap in [Tonemap::Aces, Tonemap::Reinhard] {
        let mut wild = default_post_processing();
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
