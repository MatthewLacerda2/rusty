//! The speaker-mode output stage (#546), rendered through kira (#465).

use super::*;
use crate::audio::device::backend::backend_tests::{flat, rig};
use crate::audio::device::capture::RATE;
use crate::audio::AudioBackend;
use crate::audio::{PlayParams, VoiceId};

/// The settled output peak of a constant `level` voice with the stage in `mode`.
fn settled_peak(mode: SpeakerMode, level: f32) -> f32 {
    let mut b = rig(&[level]);
    b.set_speaker_mode(mode);
    let params = PlayParams {
        looping: true,
        ..flat("clip")
    };
    assert!(b.play(VoiceId(1), &params));
    b.device().render(RATE as usize);
    let tail = b.device().render(1024);
    tail.iter().map(|f| f.0.abs()).fold(0.0, f32::max)
}

#[test]
fn home_theater_is_bit_identical() {
    for x in [0.0, 1e-7, -0.25, 0.999, 1.5, -3.0] {
        let (l, r) = shape_frame(SpeakerMode::HomeTheater, x, -x);
        assert_eq!((l.to_bits(), r.to_bits()), (x.to_bits(), (-x).to_bits()));
    }
    assert_eq!(settled_peak(SpeakerMode::HomeTheater, 0.3), 0.3);
}

#[test]
fn tv_narrows_the_dynamic_range() {
    let (quiet, loud) = (0.01, 0.5);
    let reference = settled_peak(SpeakerMode::HomeTheater, quiet)
        / settled_peak(SpeakerMode::HomeTheater, loud);
    let tv = settled_peak(SpeakerMode::Tv, quiet) / settled_peak(SpeakerMode::Tv, loud);
    assert!(tv > reference * 2.0, "tv {tv} vs reference {reference}");
    let lifted = settled_peak(SpeakerMode::Tv, quiet);
    assert!(lifted > quiet, "quiet cue lifted: {lifted}");
}

#[test]
fn tv_never_clips() {
    for level in [0.5, 1.0] {
        let peak = settled_peak(SpeakerMode::Tv, level);
        assert!(peak < 1.0, "level {level} -> {peak}");
    }
    assert!(limit(100.0) <= 1.0 && limit(-100.0) >= -1.0);
    assert_eq!(limit(0.5), 0.5);
}

#[test]
fn headphones_crossfeed_keeps_a_hard_left_signal_out_of_one_ear_only() {
    let (l, r) = shape_frame(SpeakerMode::Headphones, 1.0, 0.0);
    assert!(r > 0.0 && l > r);
    let (l, r) = shape_frame(SpeakerMode::Headphones, 0.3, 0.3);
    assert!(
        (l - 0.3).abs() < 1e-6 && (r - 0.3).abs() < 1e-6,
        "centre kept"
    );
}

#[test]
fn control_round_trips_every_mode() {
    let control = OutputControl::new(SpeakerMode::default());
    assert_eq!(control.get(), SpeakerMode::HomeTheater);
    for mode in SpeakerMode::ALL {
        control.set(mode);
        assert_eq!(control.get(), mode);
    }
}
