//! Tests for `Audio.PlayAt`'s rolloff band (#575), on the recording backend — the
//! gain a one-shot would reach the speakers with.

use glam::Vec3;

use super::*;
use crate::audio::recording::RecordingBackend;

/// The pre-master gain a shot `distance` metres down +X plays with, and after a frame.
fn shot_gain(distance: f32, rolloff: Rolloff) -> (f32, f32) {
    let (backend, rec) = RecordingBackend::new();
    let mut m = AudioMaestro::with_backend(backend);
    let shot = Shot {
        rolloff,
        ..Shot::new("shot.wav", [distance, 0.0, 0.0])
    };
    m.play_at(&shot, 0, 0);
    let voice = rec.borrow().last_voice();
    let started = rec.borrow().plays[0].1.mix.gain;
    let env = MixEnv {
        listener: Listener::new(Vec3::new(0.0, 0.0, 1.0), Vec3::X),
        ..MixEnv::default()
    };
    m.apply_mix(&env, |_| None);
    let mixed = rec.borrow().current(voice).unwrap().gain;
    (started, mixed)
}

const FAR: Rolloff = Rolloff {
    min_distance: 1.0,
    max_distance: 50.0,
};

#[test]
fn a_distant_shot_is_silent_on_the_default_band() {
    assert_eq!(shot_gain(30.0, Rolloff::default()), (0.0, 0.0));
}

#[test]
fn a_wide_band_carries_a_distant_shot_through_every_frame() {
    let (started, mixed) = shot_gain(30.0, FAR);
    assert!(started > 0.0 && mixed > 0.0, "{started} / {mixed}");
}

#[test]
fn the_band_rolls_off_linearly() {
    // Halfway across 1 → 50 m, heard from the origin at play time.
    let (started, _) = shot_gain(25.5, FAR);
    assert!((started - 0.5).abs() < 1e-5, "{started}");
    assert_eq!(shot_gain(60.0, FAR).0, 0.0, "silent beyond max_distance");
}

#[test]
fn it_sounds_until_the_last_voice_is_reaped() {
    let (backend, rec) = RecordingBackend::new();
    let mut m = AudioMaestro::with_backend(backend);
    assert!(!m.is_sounding(), "nothing played yet");
    m.play_at(&Shot::new("shot.wav", [0.0, 0.0, 0.0]), 0, 0);
    assert!(m.is_sounding());
    let voice = rec.borrow().last_voice();
    rec.borrow_mut().live.remove(&voice);
    m.apply_mix(&MixEnv::default(), |_| None);
    assert!(!m.is_sounding(), "reaped: the editor may sleep again");
}

#[test]
fn an_entity_voice_whose_clip_ended_no_longer_sounds() {
    let (backend, rec) = RecordingBackend::new();
    let mut m = AudioMaestro::with_backend(backend);
    m.play_source(7, &AudioSourceComponent::default(), [0.0; 3], 0);
    assert!(m.is_sounding());
    rec.borrow_mut().live.clear();
    assert!(m.is_source_playing(7), "its entry outlives the clip");
    assert!(!m.is_sounding(), "or the editor would never sleep again");
}
