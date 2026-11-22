use glam::Vec3;

use super::change::apart;
use super::measure::{blinks, luma, run_from};
use super::room::{anti_flicker, renderer, room, ON, WARM};
use rusty_scene::{Camera, Scene};

const FRAMES: u32 = 40;
const PAN: f32 = 0.23;
/// Measured on lavapipe: 13.3 held, 14.0 URP's.
const BLINKS: f32 = 1.0;
/// Measured on lavapipe: 1.58.
const PANNED: f32 = 3.0;

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
