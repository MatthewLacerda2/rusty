//! TAA's reactive mask and transparent motion vectors on lavapipe (#1845): the
//! issue's measurement. A 0.4 m transparent "plasma ball" (refracting, an emissive
//! swirl, a Fresnel rim) moving laterally at 9 m/s, 4 m from the camera, over a
//! plain wall, 30 frames at 60 fps; the last frame's luma over the ball, under
//! TAA and FXAA. Without the mask the ball is averaged with the wall's history and
//! loses its rim and swirl; with it (or with the ball writing its own motion) it
//! keeps them. What the scene is: `scene`.

mod coverage;
mod fsr2;
mod scene;
mod viewmodel;

use scene::{ball_frame, Ball, Stats};

use crate::components::TaaReactive;

/// How far under FXAA's the ball's p95 and brightest luma may sit under TAA: ours,
/// a few steps of a byte. Lavapipe measures the ball 13 and 24 below without the
/// mask (the issue measured 9 and 13 in its game), 5 and 11 above with it.
pub(super) const P95_TOLERANCE: f32 = 4.0;
const MAX_TOLERANCE: f32 = 8.0;

/// Whether `taa` keeps `fxaa`'s detail, within the tolerances.
fn keeps(fxaa: Stats, taa: Stats) -> bool {
    taa.p95 >= fxaa.p95 - P95_TOLERANCE && taa.max >= fxaa.max - MAX_TOLERANCE
}

fn measure(ball: Ball) -> Option<(Stats, Stats)> {
    let fxaa = ball_frame(Ball { taa: false, ..ball })?;
    let taa = ball_frame(Ball { taa: true, ..ball })?;
    eprintln!("[taa 1845] {ball:?}: FXAA {fxaa:?}, TAA {taa:?}");
    Some((fxaa, taa))
}

#[test]
fn gpu_a_moving_translucent_keeps_its_detail_under_taa() {
    let Some((fxaa, taa)) = measure(Ball::default()) else {
        return;
    };
    assert!(
        keeps(fxaa, taa),
        "the ball lost its detail: TAA {taa:?}, FXAA {fxaa:?}"
    );
}

#[test]
fn gpu_a_moving_translucent_keeps_its_detail_under_taau() {
    let scaled = Ball {
        scale: 0.6,
        ..Ball::default()
    };
    let Some((fxaa, taa)) = measure(scaled) else {
        return;
    };
    assert!(
        keeps(fxaa, taa),
        "the ball lost its detail: TAA {taa:?}, FXAA {fxaa:?}"
    );
}

#[test]
fn gpu_without_the_mask_the_history_averages_the_ball_away() {
    let off = Ball {
        reactive: TaaReactive::Off,
        ..Ball::default()
    };
    let Some((fxaa, taa)) = measure(off) else {
        return;
    };
    assert!(
        !keeps(fxaa, taa),
        "with the mask off the ball should lose its detail (the issue's case): TAA {taa:?}, FXAA {fxaa:?}"
    );
}

#[test]
fn gpu_a_translucent_writing_its_motion_keeps_its_detail() {
    let moving = Ball {
        reactive: TaaReactive::Off,
        writes_motion: true,
        ..Ball::default()
    };
    let Some((fxaa, taa)) = measure(moving) else {
        return;
    };
    assert!(
        keeps(fxaa, taa),
        "a ball writing its motion should be reprojected as a solid: TAA {taa:?}, FXAA {fxaa:?}"
    );
}
