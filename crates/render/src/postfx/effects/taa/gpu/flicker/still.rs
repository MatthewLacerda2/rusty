use super::super::super::TaaHistory;
use super::measure::{run, swing};
use super::room::{anti_flicker, renderer, room, DEFAULT, H, ON, W};
use crate::{RenderView, OFFSCREEN_FORMAT};

const FRAMES: u32 = 54;
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
