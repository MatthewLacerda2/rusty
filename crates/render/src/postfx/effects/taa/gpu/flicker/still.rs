//! The case itself (`super`): the camera held still over a jitter cycle and more,
//! URP's resolve (the anti-flicker at 0) and the anti-flicker on; and what the
//! setting costs a camera at 0: nothing held, nothing allocated.

use super::super::super::TaaHistory;
use super::measure::{run, swing};
use super::room::{anti_flicker, renderer, room, DEFAULT, H, ON, W};
use crate::{RenderView, OFFSCREEN_FORMAT};

/// Frames measured: two of the jitter's 27-frame cycles, so its 32-frame
/// horizontal one turns over too.
const FRAMES: u32 = 54;
/// The largest step a tile of the still view may take between two frames with
/// the anti-flicker on, in 255ths of perceptual luma: ours, a still view's blinks
/// reduced to near the scene's own (without SSR the bulb's edge under the jitter
/// steps 5). Measured at 0 (URP's), 0.5 (the default) and 1: lavapipe 26.2, 11.3,
/// 8.8; Metal 26.1, 11.2, 8.7; RTX 2060 26.2 and 8.8 at 0 and 1, not yet at 0.5.
const STILL: f32 = 12.0;

#[test]
fn gpu_a_still_view_under_taa_holds_a_jittered_reflection_still_with_anti_flicker() {
    let Some(mut r) = renderer() else { return };
    let none = &|_: &mut _, _| {};
    let mut step = |value| {
        swing(&run(
            &mut r,
            &mut anti_flicker(room(), value),
            (FRAMES, 1.0),
            none,
        ))
    };
    let (clamped, default, held) = (step(0.0), step(DEFAULT), step(ON));
    eprintln!(
        "[taa] a still view's largest tile step: {clamped:.1} URP's, {default:.1} at the \
         default {DEFAULT}, {held:.1} at {ON}"
    );
    assert!(
        clamped > 2.0 * STILL,
        "the case flickers under URP's resolve: {clamped:.1}"
    );
    for (value, step) in [(DEFAULT, default), (ON, held)] {
        assert!(
            step <= STILL,
            "a still view blinks under the anti-flicker at {value}: a tile steps {step:.1}"
        );
    }
}

#[test]
fn gpu_the_flicker_frames_are_held_only_while_the_anti_flicker_is_on() {
    let Some(mut r) = renderer() else { return };
    let mut v = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    let draw = |r: &mut crate::Renderer, v: &mut RenderView, value: f32| {
        let target = v.color_target_view().unwrap();
        let scene = anti_flicker(room(), value);
        r.render(v, &scene, &super::room::eye(), &target, false);
        let h = v.post_fx.targets.get::<TaaHistory>().expect("TAA ran");
        h.flickers.is_some()
    };
    assert!(
        !draw(&mut r, &mut v, 0.0),
        "a camera at 0 allocates no flicker frames"
    );
    assert!(
        draw(&mut r, &mut v, ON),
        "turned on, the history holds them"
    );
    assert!(draw(&mut r, &mut v, ON), "and keeps them while on");
    assert!(!draw(&mut r, &mut v, 0.0), "turned off, they are dropped");
}
