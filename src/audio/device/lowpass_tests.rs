//! The occlusion muffle as the kira graph renders it (#467): an open filter is
//! bit-transparent, a closed one takes the highs and keeps the lows.

use std::f32::consts::TAU;

use crate::audio::backend::{AudioBackend, PlayParams, VoiceId, VoiceMix};
use crate::audio::device::backend::backend_tests::{flat, rig};
use crate::audio::device::capture::RATE;
use crate::audio::occlusion::effect;

/// `frames` of a `hz` sine at amplitude 0.5.
fn sine(hz: f32, frames: usize) -> Vec<f32> {
    let w = TAU * hz / RATE as f32;
    (0..frames).map(|i| 0.5 * (w * i as f32).sin()).collect()
}

/// The peak of `hz` played looping at occlusion `factor`, after the filter settles.
fn peak(hz: f32, factor: f32) -> (f32, Vec<(f32, f32)>) {
    let tone = sine(hz, RATE as usize);
    let mut b = rig(&tone);
    let (gain, low_pass) = effect(factor);
    let mix = VoiceMix {
        gain,
        low_pass,
        ..VoiceMix::FLAT
    };
    let params = PlayParams {
        mix,
        looping: true,
        ..flat("clip")
    };
    assert!(b.play(VoiceId(1), &params));
    let out = b.device().render(4_800);
    let settled = &out[2_400..];
    (settled.iter().map(|f| f.0.abs()).fold(0.0, f32::max), out)
}

#[test]
fn an_unoccluded_voice_is_bit_identical_to_its_clip() {
    let (_, out) = peak(5_000.0, 0.0);
    let tone = sine(5_000.0, 64);
    let left: Vec<f32> = out[..64].iter().map(|f| f.0).collect();
    assert_eq!(left, tone);
}

#[test]
fn full_occlusion_muffles_the_highs_and_keeps_the_lows() {
    let (high, _) = peak(8_000.0, 1.0);
    let (low, _) = peak(100.0, 1.0);
    assert!(high < 0.01, "8 kHz through the wall: {high}");
    assert!(
        low > 0.5 * 0.35 * 0.9,
        "100 Hz keeps its (attenuated) level: {low}"
    );
    assert!(low < 0.5 * 0.35 * 1.01, "and is attenuated: {low}");
}
