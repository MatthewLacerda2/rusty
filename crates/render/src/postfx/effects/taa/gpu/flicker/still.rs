//! The case itself (`super`): the camera held still over a jitter cycle and more.

use super::super::super::params::Rules;
use super::room::{renderer, room, run, swing};

/// Frames measured: two of the jitter's 27-frame cycles, so its 32-frame
/// horizontal one turns over too.
const FRAMES: u32 = 54;
/// The largest step a tile of the still view may take between two frames, in
/// 255ths of perceptual luma: ours. Lavapipe measures 26 without the hold, 9 with
/// it; the scene without SSR (the bulb's own edge under the jitter) steps 5.
const STILL: f32 = 12.0;

#[test]
fn gpu_a_still_view_under_taa_holds_a_jittered_reflection_still() {
    let Some(mut r) = renderer() else { return };
    let none = &|_: &mut _, _| {};
    let held = swing(&run(&mut r, &mut room(), (FRAMES, Rules::default()), none));
    let urp = Rules {
        flicker: false,
        ..Rules::default()
    };
    let clamped = swing(&run(&mut r, &mut room(), (FRAMES, urp), none));
    eprintln!("[taa] a still view's largest tile step: {clamped:.1} clamped, {held:.1} held");
    assert!(
        clamped > 2.0 * STILL,
        "the case flickers without the hold: {clamped:.1}"
    );
    assert!(
        held <= STILL,
        "a still view blinks under TAA: a tile steps {held:.1}"
    );
}
