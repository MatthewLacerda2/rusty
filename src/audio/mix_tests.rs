//! Tests for the per-frame mix (#412), asserted on what a recording backend was
//! handed — the values that would reach the speakers.

use glam::{Quat, Vec3};

use super::*;
use crate::audio::recording::RecordingBackend;

/// A fully-3D looping source with a 1 → 11 m rolloff band.
fn spatial(time_scaled: bool) -> AudioSourceComponent {
    AudioSourceComponent {
        clip: "loop.ogg".to_string(),
        looping: true,
        spatial_blend: 1.0,
        initial_distance: 1.0,
        final_distance: 11.0,
        is_time_scaled: time_scaled,
        ..Default::default()
    }
}

/// A listener at `pos` facing -Z (right = +X), with the given clock state.
fn env_at(pos: Vec3, time_scale: f32, paused: bool) -> MixEnv {
    MixEnv {
        listener: Listener::from_transform(pos, Quat::IDENTITY),
        time_scale,
        paused,
    }
}

#[test]
fn moving_the_listener_away_lowers_the_gain_handed_over() {
    let (backend, rec) = RecordingBackend::new();
    let mut m = AudioMaestro::with_backend(backend);
    let src = spatial(true);
    m.play_source(1, &src, [0.0, 0.0, -2.0], 0);
    let voice = rec.borrow().last_voice();
    let near = rec.borrow().current(voice).unwrap().gain;
    m.apply_mix(&env_at(Vec3::new(0.0, 0.0, 4.0), 1.0, false), |_| {
        Some((src.clone(), Vec3::new(0.0, 0.0, -2.0)))
    });
    let far = rec.borrow().current(voice).unwrap().gain;
    assert!(far < near, "near {near}, far {far}");
    assert!((far - 0.5).abs() < 1e-6, "6 m in a 1→11 band is half gain");
}

#[test]
fn a_source_to_the_left_hands_over_negative_pan_equal_to_get_spatial() {
    let (backend, rec) = RecordingBackend::new();
    let mut m = AudioMaestro::with_backend(backend);
    let src = spatial(true);
    let pos = Vec3::new(-3.0, 0.0, 0.0);
    m.play_source(1, &src, pos.into(), 0);
    let env = env_at(Vec3::ZERO, 1.0, false);
    m.apply_mix(&env, |_| Some((src.clone(), pos)));
    let applied = rec.borrow().current(rec.borrow().last_voice()).unwrap();
    assert!(applied.pan < 0.0, "got {applied:?}");
    let read = m.voice_info(1, &src, pos, &env.listener).spatial;
    assert_eq!((applied.gain, applied.pan), (read.gain, read.pan));
}

#[test]
fn master_volume_is_folded_into_the_handed_gain() {
    let (backend, rec) = RecordingBackend::new();
    let mut m = AudioMaestro::with_backend(backend);
    m.play_source(1, &spatial(true), [0.0; 3], 0);
    m.set_master_volume(0.25);
    let voice = rec.borrow().last_voice();
    assert_eq!(rec.borrow().current(voice).unwrap().gain, 0.25);
    assert_eq!(
        m.voice_mix(1).unwrap().gain,
        1.0,
        "the stored mix is pre-master"
    );
}

#[test]
fn time_scale_sets_the_rate_of_time_scaled_voices_only() {
    let (backend, rec) = RecordingBackend::new();
    let mut m = AudioMaestro::with_backend(backend);
    m.play_source(1, &spatial(true), [0.0; 3], 0);
    m.play_source(2, &spatial(false), [0.0; 3], 0);
    m.apply_mix(&env_at(Vec3::ZERO, 0.5, false), |_| None);
    assert_eq!(m.voice_mix(1).unwrap().speed, 0.5);
    assert_eq!(m.voice_mix(2).unwrap().speed, 1.0);
    let scaled = rec.borrow().plays[0].0;
    assert_eq!(rec.borrow().current(scaled).unwrap().speed, 0.5);
}

#[test]
fn zero_time_scale_or_pause_holds_only_time_scaled_voices_then_resumes() {
    let mut m = AudioMaestro::default();
    m.play_source(1, &spatial(true), [0.0; 3], 0);
    m.play_source(2, &spatial(false), [0.0; 3], 0);
    for env in [
        env_at(Vec3::ZERO, 0.0, false),
        env_at(Vec3::ZERO, 1.0, true),
    ] {
        m.apply_mix(&env, |_| None);
        assert!(m.voice_mix(1).unwrap().paused, "{env:?}");
        assert!(!m.voice_mix(2).unwrap().paused, "{env:?}");
        m.apply_mix(&env_at(Vec3::ZERO, 1.0, false), |_| None);
        assert!(!m.voice_mix(1).unwrap().paused, "resumes after {env:?}");
        assert!(m.is_source_playing(1), "paused, not stopped");
    }
}

#[test]
fn an_unchanged_mix_is_not_resent() {
    let (backend, rec) = RecordingBackend::new();
    let mut m = AudioMaestro::with_backend(backend);
    m.play_source(1, &spatial(true), [0.0; 3], 0);
    m.apply_mix(&MixEnv::default(), |_| None);
    assert!(rec.borrow().mixes.is_empty());
}

#[test]
fn a_voice_started_between_frames_uses_the_last_frames_listener() {
    let (backend, rec) = RecordingBackend::new();
    let mut m = AudioMaestro::with_backend(backend);
    m.apply_mix(&env_at(Vec3::new(3.0, 0.0, 0.0), 1.0, false), |_| None);
    m.play_source(1, &spatial(true), [0.0; 3], 0);
    assert!(
        rec.borrow().plays[0].1.mix.pan < 0.0,
        "listener is to the right"
    );
}

#[test]
fn play_at_is_spatialized_and_reaped_when_the_backend_finishes_it() {
    let (backend, rec) = RecordingBackend::new();
    let mut m = AudioMaestro::with_backend(backend);
    m.play_at("boom.wav", [-2.0, 0.0, 0.0], 1.0, Rolloff::default(), 0, 0);
    let voice = rec.borrow().last_voice();
    assert!(rec.borrow().plays[0].1.mix.pan < 0.0);
    m.apply_mix(&env_at(Vec3::ZERO, 0.5, false), |_| None);
    assert_eq!(rec.borrow().current(voice).unwrap().speed, 0.5);
    assert_eq!(m.live_oneshots(), 1);
    rec.borrow_mut().live.remove(&voice);
    m.apply_mix(&MixEnv::default(), |_| None);
    assert_eq!(m.live_oneshots(), 0);
}
