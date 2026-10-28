//! Contrast on lavapipe (#1479): URP's, in LogC around mid-grey before the tonemap,
//! so a mild boost darkens dark metal without clipping it to black.

use super::{chain, W};
use crate::components::PostProcessingComponent;
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
    (row[1], row[2], row[3]) = ([0.05; 3], [0.22; 3], [2.0; 3]);
    let neutral = c.run(&row, 0.0, Some(&default_post_processing()));
    let punchy = c.run(&row, 0.0, Some(&at(1.12)));
    let flat = c.run(&row, 0.0, Some(&at(0.5)));
    let g = |out: &[[u8; 4]], i: usize| out[i][1];
    let head = |out: &[[u8; 4]]| out[..4].iter().map(|p| p[1]).collect::<Vec<_>>();
    let (n, p, f) = (head(&neutral), head(&punchy), head(&flat));
    eprintln!("[contrast] black, 0.05, 0.22, 2.0: 1 {n:?}, 1.12 {p:?}, 0.5 {f:?}");

    assert!(g(&punchy, 1) > 0, "1.12 keeps 0.05 above black");
    assert!(g(&punchy, 1) < g(&neutral, 1), "1.12 darkens the shadows");
    assert!(g(&punchy, 3) > g(&neutral, 3), "1.12 lifts the highlights");
    let pivot = i16::from(g(&punchy, 2)) - i16::from(g(&neutral, 2));
    assert!(pivot.abs() <= 2, "mid-grey stays put: {pivot}");
    assert_eq!((g(&neutral, 0), g(&punchy, 0)), (0, 0), "black stays black");
    assert!(g(&flat, 1) > g(&neutral, 1), "0.5 lifts the shadows");
    assert!(
        g(&flat, 3) < g(&neutral, 3),
        "0.5 pulls the highlights down"
    );
    // URP lifts black too (to ~50 here); the linear-0.5 pivot lifted it to ~137.
    assert!(
        g(&flat, 0) < 70,
        "0.5 does not wash black out: {}",
        g(&flat, 0)
    );
}

#[test]
fn gpu_contrast_one_is_the_ungraded_image_to_the_byte() {
    let Some(mut c) = chain() else { return };
    let row: Vec<[f32; 3]> = (0..W)
        .map(|i| [0.002 * 1.15f32.powi(i as i32); 3])
        .collect();
    let none = c.run(&row, 0.0, Some(&default_post_processing()));
    // Bake a contrast first, so the neutral one below is a rebake, not a cache hit.
    c.run(&row, 0.0, Some(&at(1.3)));
    let mut vc = default_post_processing();
    ops::grading::color_adjustments::set_contrast(&mut vc, 1.0);
    assert_eq!(c.run(&row, 0.0, Some(&vc)), none);
}
