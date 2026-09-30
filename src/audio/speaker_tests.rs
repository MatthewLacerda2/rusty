use serde_json::Value;

use super::*;

#[test]
fn default_is_home_theater() {
    assert_eq!(SpeakerMode::default(), SpeakerMode::HomeTheater);
}

#[test]
fn names_round_trip_and_parse_ignores_case() {
    for mode in SpeakerMode::ALL {
        assert_eq!(SpeakerMode::parse(mode.name()), Some(mode));
    }
    assert_eq!(SpeakerMode::parse("TV"), Some(SpeakerMode::Tv));
    assert_eq!(SpeakerMode::parse("surround"), None);
}

#[test]
fn headphones_narrow_a_hard_pan_other_modes_pass_through() {
    let hard_left = VoiceMix {
        pan: -1.0,
        ..VoiceMix::FLAT
    };
    let narrowed = SpeakerMode::Headphones.shape(hard_left);
    assert!(
        narrowed.pan.abs() < 1.0,
        "right ear not silent: {narrowed:?}"
    );
    assert_eq!(narrowed.gain, hard_left.gain);
    assert_eq!(SpeakerMode::Tv.shape(hard_left), hard_left);
    assert_eq!(SpeakerMode::HomeTheater.shape(hard_left), hard_left);
}

#[test]
fn storage_round_trips_every_mode() {
    for mode in SpeakerMode::ALL {
        let mut storage = Storage::new();
        mode.store(&mut storage);
        assert_eq!(SpeakerMode::load(&storage), mode);
    }
}

#[test]
fn missing_or_garbage_value_loads_the_default() {
    let mut storage = Storage::new();
    assert_eq!(SpeakerMode::load(&storage), SpeakerMode::HomeTheater);
    storage.set(AUDIO_NAMESPACE, SPEAKER_MODE_KEY, Value::from(7));
    assert_eq!(SpeakerMode::load(&storage), SpeakerMode::HomeTheater);
    storage.set(
        AUDIO_NAMESPACE,
        SPEAKER_MODE_KEY,
        Value::from("quadraphonic"),
    );
    assert_eq!(SpeakerMode::load(&storage), SpeakerMode::HomeTheater);
}
