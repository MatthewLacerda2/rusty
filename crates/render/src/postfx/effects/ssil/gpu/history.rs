//! SSIL's history (#1750): a light switched off fades out within a few frames, a
//! turning camera doesn't crawl, a cut starts over, and a NaN in last frame's light
//! never spreads.

use crate::test_gpu::ssil::{draw, eye, on, patch, patch_at, renderer, scene, view, FLOOR, LAMP};
use rusty_components::Ssil;

/// Frames after a light goes off by which its bounce is under a tenth of what it
/// was.
const FADE_FRAMES: u32 = 2;

/// How much a floor point's bounce may change from one frame to the next while the
/// camera turns, as a share of the bounce.
const CRAWL: f32 = 0.1;

/// The floor's red with SSIL on less with it off: the bounce alone.
fn bounce(on_frame: &[u8], off_frame: &[u8]) -> f32 {
    patch(on_frame, FLOOR)[0] - patch(off_frame, FLOOR)[0]
}

#[test]
fn gpu_a_light_switched_off_fades_within_two_frames() {
    let Some(mut r) = renderer() else { return };
    let (mut lit, lamp) = scene(on(1.0), LAMP);
    let (mut v, cam) = (view(&r), eye(0.0));
    let off_scene = |light| scene(Ssil::default(), light).0;
    let mut frame = Vec::new();
    for _ in 0..6 {
        frame = draw(&mut r, &mut v, &lit, &cam);
    }
    let before = bounce(
        &frame,
        &crate::test_gpu::ssil::settled(&mut r, &off_scene(LAMP), 1),
    );
    lit.world.light_mut(lamp).unwrap().intensity = 0.0;
    for _ in 0..FADE_FRAMES {
        frame = draw(&mut r, &mut v, &lit, &cam);
    }
    let dark = crate::test_gpu::ssil::settled(&mut r, &off_scene(0.0), 1);
    let after = bounce(&frame, &dark);
    eprintln!("[ssil] bounce {before} lit, {after} {FADE_FRAMES} frames after");
    assert!(
        before > 0.0 && after <= before * 0.1,
        "it fades, not sticks"
    );
}

#[test]
fn gpu_a_turning_camera_does_not_crawl() {
    let Some(mut r) = renderer() else { return };
    let (lit, dark) = (scene(on(1.0), LAMP).0, scene(Ssil::default(), LAMP).0);
    let (mut v, mut plain) = (view(&r), view(&r));
    let mut seen = Vec::new();
    // A steady turn, half a degree a frame: the floor point slides across the
    // screen, and its bounce, read where it lands each frame, should hold still.
    for i in 0..24 {
        let yaw = i as f32 * 0.5;
        let cam = eye(yaw);
        let (on, off) = (
            draw(&mut r, &mut v, &lit, &cam),
            draw(&mut r, &mut plain, &dark, &cam),
        );
        if i >= 12 {
            seen.push(patch_at(&on, FLOOR, yaw)[0] - patch_at(&off, FLOOR, yaw)[0]);
        }
    }
    let mean = seen.iter().sum::<f32>() / seen.len() as f32;
    let step = seen
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0, f32::max);
    eprintln!("[ssil] the floor point's bounce while turning: {seen:?}");
    assert!(
        mean > 0.0 && step <= mean * CRAWL,
        "a frame-to-frame step of {step} on {mean}"
    );
}

#[test]
fn gpu_a_nan_in_last_frames_light_never_spreads() {
    let Some(mut r) = renderer() else { return };
    let (lit, _) = scene(on(1.0), LAMP);
    let (mut v, cam) = (view(&r), eye(0.0));
    for _ in 0..3 {
        draw(&mut r, &mut v, &lit, &cam);
    }
    v.post_fx.poison_ssil_source(&r.queue);
    draw(&mut r, &mut v, &lit, &cam);
    let result = v.post_fx.ssil_result(&r.device, &r.queue);
    assert!(
        result.iter().all(|c| c.is_finite()),
        "the result stays finite"
    );
    assert!(
        result.iter().any(|c| c.max_element() > 0.0),
        "and still lit"
    );
}

#[test]
fn gpu_a_cut_starts_the_history_over() {
    let Some(mut r) = renderer() else { return };
    let (lit, _) = scene(on(1.0), LAMP);
    let (mut v, cam) = (view(&r), eye(0.0));
    for _ in 0..3 {
        draw(&mut r, &mut v, &lit, &cam);
    }
    // Another scene is another history key: its first frame gathers nothing.
    let (other, _) = scene(on(1.0), LAMP);
    let first = draw(&mut r, &mut v, &other, &cam);
    let off = crate::test_gpu::ssil::settled(&mut r, &scene(Ssil::default(), LAMP).0, 1);
    assert!(
        first == off,
        "a cut adds nothing, rather than last scene's light"
    );
}
