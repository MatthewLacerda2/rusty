//! The speaker mode through the maestro (#546), asserted on what a recording backend
//! was handed — the per-voice half of the output shaping.

use glam::Vec3;

use super::*;
use crate::audio::recording::RecordingBackend;
use crate::audio::MixEnv;

/// A fully-3D looping source hard left of a listener at the origin.
fn hard_left() -> (AudioSourceComponent, [f32; 3]) {
    let src = AudioSourceComponent {
        clip: "loop.ogg".to_string(),
        looping: true,
        spatial_blend: 1.0,
        ..Default::default()
    };
    (src, [-0.5, 0.0, 0.0])
}

#[test]
fn default_home_theater_hands_over_the_unshaped_mix() {
    let (backend, rec) = RecordingBackend::new();
    let mut m = AudioMaestro::with_backend(backend);
    assert_eq!(m.speaker_mode(), SpeakerMode::HomeTheater);
    let (src, at) = hard_left();
    m.set_master_volume(0.7);
    m.play_source(1, &src, at, 0);
    let expected = mix::resolve_voice(&MixEnv::default(), &src, 1.0, Vec3::from(at), 0.0);
    let handed = rec.borrow().current(rec.borrow().last_voice()).unwrap();
    assert_eq!(
        handed,
        expected.with_master(0.7),
        "bit-identical to the pre-#546 fold"
    );
    assert_eq!(handed.pan, -1.0);
}

#[test]
fn headphones_narrow_live_and_new_voices_and_tell_the_bus() {
    let (backend, rec) = RecordingBackend::new();
    let mut m = AudioMaestro::with_backend(backend);
    let (src, at) = hard_left();
    m.play_source(1, &src, at, 0);
    let live = rec.borrow().last_voice();

    m.set_speaker_mode(SpeakerMode::Headphones);
    assert_eq!(rec.borrow().speaker_modes, vec![SpeakerMode::Headphones]);
    let narrowed = rec.borrow().current(live).unwrap();
    assert!(
        narrowed.pan.abs() < 1.0 && narrowed.pan < 0.0,
        "{narrowed:?}"
    );

    let shot = mix::Shot::new("boom.ogg", at);
    m.play_at(&shot, 0, 1);
    let fresh = rec.borrow().current(rec.borrow().last_voice()).unwrap();
    assert_eq!(fresh.pan, narrowed.pan, "a new voice starts narrowed too");
    assert_eq!(
        m.voice_mix(1).unwrap().pan,
        -1.0,
        "stored mix stays unshaped"
    );
}

#[test]
fn tv_leaves_the_voice_mix_to_the_bus() {
    let (backend, rec) = RecordingBackend::new();
    let mut m = AudioMaestro::with_backend(backend);
    let (src, at) = hard_left();
    m.play_source(1, &src, at, 0);
    let voice = rec.borrow().last_voice();
    let before = rec.borrow().current(voice).unwrap();
    m.set_speaker_mode(SpeakerMode::Tv);
    assert_eq!(rec.borrow().current(voice).unwrap(), before);
    assert_eq!(rec.borrow().speaker_modes, vec![SpeakerMode::Tv]);
}

#[test]
fn a_swapped_in_backend_takes_the_current_mode() {
    let mut m = AudioMaestro::default();
    m.set_speaker_mode(SpeakerMode::Tv);
    let (backend, rec) = RecordingBackend::new();
    m.set_backend(backend);
    assert_eq!(rec.borrow().speaker_modes, vec![SpeakerMode::Tv]);
    assert_eq!(m.speaker_mode(), SpeakerMode::Tv);
}
