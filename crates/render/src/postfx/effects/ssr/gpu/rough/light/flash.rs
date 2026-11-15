//! A one-frame flash moves the radius field's brightness reference, the exposure
//! the frame is shown at, no further than that exposure moves (#2249): not at all
//! on the flash's own frame (SSR reads the EV the meter left before it), and
//! after it exactly as the exposure did.

use super::reference::{apart, draw, outside, scene, the_left_wall, AUTO};
use crate::{RenderView, OFFSCREEN_FORMAT};
use rusty_components::ExposureMode;

/// How far the SSR-resolved frame outside a flash's own columns may move, per
/// pixel, in 255ths of perceptual luma, on the frame a one-frame flash shows, and
/// after it against the room fixed at the EV the flash left. Measured on
/// lavapipe with `ENERGY_FLOOR` 0.02: 0.00 and 0.00; 0.30 on the flash's frame
/// with the colour pyramid's 1×1 level as the reference.
const FLASH: f32 = 0.1;

#[test]
fn gpu_a_one_frame_flash_moves_the_radius_field_no_more_than_the_exposure() {
    let size = (super::W, super::H);
    let Some(mut r) = crate::test_gpu::headless_or_skip(size.0, size.1) else {
        return;
    };
    let mut v = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, size.0, size.1);
    let skip = 0..150;
    let (mut calm, mut flash) = (
        scene(AUTO, the_left_wall(0.0)),
        scene(AUTO, the_left_wall(50.0)),
    );
    let mut ev = None;
    for f in 0..4 {
        ev = draw(&mut r, &mut v, (&mut calm, 0.0), f);
    }
    let still = outside(&r, &v, size, skip.clone());
    let moved = draw(&mut r, &mut v, (&mut flash, 0.0), 4).expect("metered");
    let during = apart(&still, &outside(&r, &v, size, skip.clone()));
    draw(&mut r, &mut v, (&mut calm, 0.0), 5);
    let next = outside(&r, &v, size, skip.clone());
    // The calm room at the EV the flash left, fixed: the field the exposure moved to.
    let mut fixed = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, size.0, size.1);
    let mut held = scene((ExposureMode::Fixed, moved), the_left_wall(0.0));
    draw(&mut r, &mut fixed, (&mut held, 0.0), 0);
    let follows = apart(&next, &outside(&r, &fixed, size, skip.clone()));
    let ev = ev.expect("metered");
    eprintln!("[ssr] a one-frame flash: the exposure moves {:.3} EV; the frame beside it moves {during:.2} during it, {:.2} after it, {follows:.2} from the room fixed at the moved EV", moved - ev, apart(&still, &next));
    assert!(
        during <= FLASH,
        "a flash re-weights the radius field the frame it shows: {during:.2}"
    );
    assert!(
        follows <= FLASH,
        "after a flash the radius field moves more than the exposure: {follows:.2}"
    );
}
