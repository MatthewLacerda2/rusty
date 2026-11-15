//! A lamp crossing the part of a 1080p frame the colour pyramid's 1×1 level
//! leaves out (x ≥ 1024) leaves the radius field's brightness reference, the
//! exposure the frame is shown at, where it was (#2249): the reference covers the
//! whole frame.

use super::reference::{apart, draw, hanging, outside, scene, wall_x, AUTO};
use crate::{RenderView, OFFSCREEN_FORMAT};

/// How far the SSR-resolved frame outside a lamp's own columns may move, per
/// pixel, in 255ths of perceptual luma, as a lamp glowing 2000 crosses x = 1024
/// at 1080p. Measured on lavapipe with `ENERGY_FLOOR` 0.02: 0.17; 19.3 with the
/// colour pyramid's 1×1 level as the reference, which covers only x < 1024 there.
const STEADY: f32 = 0.5;

/// How far the crossing test turns the eye: the bulb's reflection, whose
/// neighbourhood the reference weighs (bright hits beside dim ones), well left
/// of the columns it skips.
const TURN: f32 = 15.0;

#[test]
fn gpu_a_lamp_crossing_the_frames_right_part_leaves_the_radius_field_alone() {
    let size = (1920, 1080);
    let Some(mut r) = crate::test_gpu::headless_or_skip(size.0, size.1) else {
        return;
    };
    let mut v = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, size.0, size.1);
    let (left, right) = (wall_x(990.0, size, TURN), wall_x(1060.0, size, TURN));
    let skip = 800..1250;
    let mut before = scene(AUTO, hanging(left, 2000.0));
    for f in 0..3 {
        draw(&mut r, &mut v, (&mut before, TURN), f);
    }
    let still = outside(&r, &v, size, skip.clone());
    let mut after = scene(AUTO, hanging(right, 2000.0));
    let mut most = 0.0_f32;
    for f in 3..6 {
        draw(&mut r, &mut v, (&mut after, TURN), f);
        most = most.max(apart(&still, &outside(&r, &v, size, skip.clone())));
    }
    eprintln!("[ssr] a lamp crossing x = 1024 at 1080p moves the frame beside it by {most:.2}");
    assert!(
        most <= STEADY,
        "the radius field's brightness reference misses part of the frame: {most:.2}"
    );
}
