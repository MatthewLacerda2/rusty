//! The doppler rate glides (#1749): a voice retuned from rate 1 to 2 speeds its
//! playhead up over a few ms, never in one step, and a new voice starts on its rate.

use super::*;
use crate::audio::backend::{AudioBackend, PlayParams, VoiceId, VoiceMix};
use crate::audio::device::backend::backend_tests::{flat, rig};
use crate::audio::device::capture::{Capture, RATE};
use crate::audio::device::decode::CachedClip;
use crate::audio::device::KiraBackend;

/// The ramp's value per clip frame: small, so the output never clips.
const SCALE: f32 = 1.0 / 131_072.0;

/// A backend playing a mono ramp (each sample its index × [`SCALE`], so the left
/// channel reads the playhead) at `mix`.
fn ramp(mix: VoiceMix) -> KiraBackend<Capture> {
    let mut backend = rig(&[]);
    let clip: Vec<f32> = (0..RATE).map(|i| i as f32 * SCALE).collect();
    backend
        .cache()
        .insert("ramp", CachedClip::new(1, RATE, clip));
    let params = PlayParams {
        mix,
        ..flat("ramp")
    };
    assert!(backend.play(VoiceId(1), &params));
    backend
}

/// The playhead's advance per output sample over the next `n` samples.
fn steps(backend: &mut KiraBackend<Capture>, n: usize) -> Vec<f32> {
    let out = backend.device().render(n + 1);
    out.windows(2).map(|w| (w[1].0 - w[0].0) / SCALE).collect()
}

#[test]
fn a_doppler_step_glides_the_playhead_rate() {
    let mut backend = ramp(VoiceMix::FLAT);
    assert!(steps(&mut backend, 64)
        .iter()
        .all(|s| (s - 1.0).abs() < 1e-2));
    backend.set_mix(VoiceId(1), &VoiceMix::FLAT.with_doppler(2.0));
    let settle = (5.0 * SMOOTHING * f64::from(RATE)) as usize;
    let glided = steps(&mut backend, settle);
    assert!(glided[0] < 1.1, "no jump: {}", glided[0]);
    assert!(
        glided.windows(2).all(|w| w[1] >= w[0] - 1e-2),
        "it only speeds up"
    );
    assert!(
        (glided[settle - 1] - 2.0).abs() < 0.02,
        "{}",
        glided[settle - 1]
    );
}

#[test]
fn a_new_voice_starts_on_its_doppler_rate() {
    let mut backend = ramp(VoiceMix::FLAT.with_doppler(1.5));
    let first = steps(&mut backend, 8);
    assert!(first.iter().all(|s| (s - 1.5).abs() < 1e-2), "{first:?}");
}
