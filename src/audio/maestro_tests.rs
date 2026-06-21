//! Tests for `AudioMaestro` over the `NullBackend` (no device needed).

use super::*;
use crate::audio::introspection::AudioEventKind;
use crate::audio::spatial::Listener;
use glam::{Quat, Vec3};

fn source(clip: &str, volume: f32, looping: bool) -> AudioSourceComponent {
    AudioSourceComponent {
        clip: clip.to_string(),
        volume,
        looping,
        ..Default::default()
    }
}

/// A listener at the origin facing -Z (right axis = +X), the default test camera.
fn listener() -> Listener {
    Listener::from_transform(Vec3::ZERO, Quat::IDENTITY)
}

#[test]
fn play_source_marks_playing_and_logs() {
    let mut m = AudioMaestro::default();
    assert!(!m.is_source_playing(7));
    m.play_source(7, &source("a.ogg", 1.0, true), [1.0, 2.0, 3.0], 5);
    assert!(m.is_source_playing(7));
    let ev = m.events().last().unwrap();
    assert_eq!(ev.kind, AudioEventKind::Play);
    assert_eq!(ev.entity, 7);
    assert_eq!(ev.clip, "a.ogg");
    assert_eq!(ev.position, [1.0, 2.0, 3.0]);
    assert_eq!(ev.tick, 5);
}

#[test]
fn stop_source_clears_playing_and_optionally_logs() {
    let mut m = AudioMaestro::default();
    m.play_source(1, &source("a.ogg", 1.0, false), [0.0; 3], 0);
    m.stop_source(1, [0.0; 3], 9, true);
    assert!(!m.is_source_playing(1));
    let ev = m.events().last().unwrap();
    assert_eq!(ev.kind, AudioEventKind::Stop);
    assert_eq!(ev.entity, 1);
    assert_eq!(ev.tick, 9);
}

#[test]
fn replaying_a_source_does_not_log_a_phantom_stop() {
    let mut m = AudioMaestro::default();
    m.play_source(1, &source("a.ogg", 1.0, false), [0.0; 3], 0);
    m.play_source(1, &source("b.ogg", 1.0, false), [0.0; 3], 1);
    // Two Play events, no interleaved Stop (the internal replace suppresses it).
    let kinds: Vec<_> = m.events().iter().map(|e| e.kind).collect();
    assert_eq!(kinds, vec![AudioEventKind::Play, AudioEventKind::Play]);
}

#[test]
fn master_volume_is_clamped() {
    let mut m = AudioMaestro::default();
    m.set_master_volume(5.0);
    assert_eq!(m.master_volume(), 1.0);
    m.set_master_volume(-1.0);
    assert_eq!(m.master_volume(), 0.0);
}

#[test]
fn play_at_logs_a_oneshot_without_an_entity_voice() {
    let mut m = AudioMaestro::default();
    m.play_at("boom.wav", [4.0, 0.0, -2.0], 0.8, 42, 11);
    // One-shot leaves no live entity voice…
    assert!(!m.is_source_playing(42));
    // …but its only trace is the event log.
    let ev = m.events().last().unwrap();
    assert_eq!(ev.kind, AudioEventKind::PlayAt);
    assert_eq!(ev.entity, 42);
    assert_eq!(ev.clip, "boom.wav");
    assert_eq!(ev.position, [4.0, 0.0, -2.0]);
    assert_eq!(ev.volume, 0.8);
    assert_eq!(ev.tick, 11);
}

#[test]
fn set_source_volume_only_affects_live_voices() {
    let mut m = AudioMaestro::default();
    // No live voice yet — a no-op, no panic.
    m.set_source_volume(3, 0.5);
    m.play_source(3, &source("a.ogg", 1.0, true), [0.0; 3], 0);
    m.set_source_volume(3, 0.25);
    // Still playing; volume change is reflected via the roster's source view.
    let info = m.voice_info(3, &source("a.ogg", 0.25, true), Vec3::ZERO, &listener());
    assert!(info.playing);
    assert_eq!(info.volume, 0.25);
}

#[test]
fn stop_all_clears_every_voice() {
    let mut m = AudioMaestro::default();
    m.play_source(1, &source("a.ogg", 1.0, true), [0.0; 3], 0);
    m.play_source(2, &source("b.ogg", 1.0, true), [0.0; 3], 0);
    m.stop_all();
    assert!(!m.is_source_playing(1));
    assert!(!m.is_source_playing(2));
}

#[test]
fn voice_info_reflects_source_fields() {
    let mut m = AudioMaestro::default();
    let s = source("music.ogg", 0.7, true);
    m.play_source(8, &s, [0.0; 3], 0);
    let info = m.voice_info(8, &s, Vec3::ZERO, &listener());
    assert_eq!(info.entity, 8);
    assert_eq!(info.clip, "music.ogg");
    assert!(info.looping);
    assert!(info.is_time_scaled);
    assert!(info.playing);
}
