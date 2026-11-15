//! What the anti-flicker (`super`) does to a slow pan: the camera turning about a
//! pixel a frame over the flickering ceiling, the bulb's edge crossing the view. A
//! pixel's hold falls off as it moves from `params::STILL_PIXELS` to
//! `MOVING_PIXELS` a frame, so a pan this slow keeps a little of it: it may blink
//! no more than URP's resolve, and its image may sit no farther from URP's than a
//! trail behind an edge would show.

use glam::Vec3;

use super::change::apart;
use super::measure::{blinks, luma, run_from};
use super::room::{anti_flicker, renderer, room, ON, WARM};
use rusty_scene::{Camera, Scene};

/// Frames measured.
const FRAMES: u32 = 40;
/// The pan, degrees of yaw a frame: about a pixel a frame at the view's centre
/// (45° of fov over 240 rows, the view pitched 30° up); at twice it every pixel
/// moves `MOVING_PIXELS` and the resolve is URP's to the bit.
const PAN: f32 = 0.23;
/// How much more a tile of the panned view may blink under the hold than under
/// URP's, in 255ths: ours, a level (the hold adds no blink).
/// Measured: RTX 2060 13.3 held, 14.0 URP's.
const BLINKS: f32 = 1.0;
/// How far a tile of the panned view under the hold may sit from URP's, in 255ths:
/// ours, under the trail a moving cube leaves (`change`, 3 to 4 levels), so the
/// hold drags no edge. Measured: RTX 2060 1.6.
const PANNED: f32 = 3.0;

/// The seat of frame `f`: `super::room::eye` turning by [`PAN`] a frame, the bulb
/// crossing the view's middle while it is measured.
fn panning(f: u32) -> Camera {
    let yaw = -90.0 - PAN * (WARM + FRAMES / 2) as f32 + PAN * f as f32;
    Camera::new(Vec3::new(0.0, 1.6, 0.0), yaw, 30.0)
}

#[test]
fn gpu_a_slow_pan_over_a_flickering_ceiling_blinks_and_trails_no_more_than_urps() {
    let Some(mut r) = renderer() else { return };
    let none = &|_: &mut Scene, _: u32| {};
    let mut pan = |value| {
        let scene = &mut anti_flicker(room(), value);
        run_from(&mut r, scene, (FRAMES, 1.0), none, (&panning, luma))
    };
    let (held, urp) = (pan(ON), pan(0.0));
    let (blinked, clamped, gap) = (blinks(&held), blinks(&urp), apart(&held, &urp));
    eprintln!(
        "[taa] a pixel-a-frame pan's largest blink: {blinked:.1} held, {clamped:.1} URP's; \
         the two apart by {gap:.2}"
    );
    assert!(
        blinked <= clamped + BLINKS,
        "the hold blinks on a pan: {blinked:.1} against {clamped:.1}"
    );
    assert!(gap <= PANNED, "the hold drags the panned image: {gap:.2}");
}
