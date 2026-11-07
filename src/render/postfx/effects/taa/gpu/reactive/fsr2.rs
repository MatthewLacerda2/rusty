//! #1845's plasma ball under FSR2 (#1848): the reactive mask is FSR2's reactive
//! input, so the moving translucent keeps its detail there, and loses more of it
//! with the mask off.
//!
//! FSR2 trusts a reactive pixel's history less, never not at all: at its full 0.9
//! the history keeps a tenth of its accumulated weight (`ComputeBaseAccumulationWeight`),
//! so the ball's brightest pixel settles under FXAA's by more than TAA's resolve
//! lets it (lavapipe: 11 under with the mask, 19 without; the 95th percentile 1 and
//! 7 under). So the test holds the body (the 95th percentile, TAA's tolerance) and
//! that the mask keeps more than no mask on both measures.

use super::scene::{Ball, Stats};
use super::{measure, P95_TOLERANCE};
use crate::components::TaaReactive;
use crate::core::settings::upscaler::Upscaler;

/// The ball at a render scale of 0.6 (the issue's game's), FSR2 resolving: FXAA's
/// frame and FSR2's.
fn fsr2(reactive: TaaReactive) -> Option<(Stats, Stats)> {
    measure(Ball {
        scale: 0.6,
        upscaler: Upscaler::Fsr2,
        reactive,
        ..Ball::default()
    })
}

#[test]
fn gpu_a_moving_translucent_keeps_its_detail_under_fsr2() {
    let (Some((fxaa, on)), Some((_, off))) = (fsr2(TaaReactive::Auto), fsr2(TaaReactive::Off))
    else {
        return;
    };
    assert!(
        on.p95 >= fxaa.p95 - P95_TOLERANCE,
        "the ball's body faded under FSR2: {on:?}, FXAA {fxaa:?}"
    );
    assert!(
        on.p95 > off.p95 && on.max > off.max,
        "the mask keeps more of the ball than no mask: {on:?} vs {off:?}"
    );
}
