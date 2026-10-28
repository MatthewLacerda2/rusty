//! Saturation on lavapipe (#1479): on the scene-linear colour before the tonemap, as
//! URP has it, with a channel pushed below zero held at black.

use super::{chain, W};
use crate::components::PostProcessingComponent;
use crate::scene::authoring::default_post_processing;
use crate::scene::authoring::rendering::post_processing as ops;

/// The default (ACES) volume at `saturation`.
fn at(saturation: f32) -> PostProcessingComponent {
    let mut vc = default_post_processing();
    ops::grading::color_adjustments::set_saturation(&mut vc, saturation);
    vc
}

#[test]
fn gpu_saturation_runs_before_the_tonemap_and_holds_channels_at_black() {
    let Some(mut c) = chain() else { return };
    // A bright red with a little green and blue in it.
    let row = vec![[2.0f32, 0.1, 0.1]; W as usize];
    let neutral = c.run(&row, 0.0, Some(&at(1.0)))[0];
    let vivid = c.run(&row, 0.0, Some(&at(1.5)))[0];
    let grey = c.run(&row, 0.0, Some(&at(0.0)))[0];
    eprintln!("[saturation] 1 {neutral:?}, 1.5 {vivid:?}, 0 {grey:?}");

    // Before the tonemap, 1.5 takes green and blue to 0.50 + 1.5 * (0.1 - 0.50) < 0,
    // which clamps to black. After it (or unclamped into ACES) they stay well lit.
    assert_eq!((vivid[1], vivid[2]), (0, 0), "1.5 empties green and blue");
    assert!(vivid[0] >= neutral[0], "1.5 keeps the red");
    assert!(neutral[1] > 0, "the neutral grade keeps some green");
    assert_eq!((grey[0], grey[2]), (grey[1], grey[1]), "0 is grey");
}
