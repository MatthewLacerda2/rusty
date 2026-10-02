//! Tests for the voice's stereo rule and balance pan law (#412, ported to kira by
//! #465). The rendered behaviour — through kira's tracks — is `backend_tests.rs`.

use super::*;

#[test]
fn mono_panned_hard_left_silences_the_right_channel() {
    assert_eq!(mix_frame(&[0.5], -1.0, 1.0), (0.5, 0.0));
    assert_eq!(mix_frame(&[-0.25], -1.0, 1.0), (-0.25, 0.0));
}

#[test]
fn centred_mono_plays_at_unity_on_both_channels() {
    assert_eq!(mix_frame(&[0.5], 0.0, 1.0), (0.5, 0.5));
}

#[test]
fn half_pan_right_fades_the_left_channel_linearly() {
    assert_eq!(mix_frame(&[1.0], 0.5, 1.0), (0.5, 1.0));
}

#[test]
fn fully_spatial_stereo_is_downmixed_before_panning() {
    // l = 1, r = 0 → mono 0.5 on both channels when centred.
    assert_eq!(mix_frame(&[1.0, 0.0], 0.0, 1.0), (0.5, 0.5));
}

#[test]
fn pure_2d_stereo_keeps_its_image_untouched() {
    assert_eq!(mix_frame(&[1.0, 0.0], 0.0, 0.0), (1.0, 0.0));
    assert_eq!(mix_frame(&[0.25, -0.5], 0.0, 0.0), (0.25, -0.5));
}

#[test]
fn half_blend_stereo_lerps_toward_the_downmix() {
    // l = 1, r = 0, mono 0.5 → halfway: (0.75, 0.25).
    assert_eq!(mix_frame(&[1.0, 0.0], 0.0, 0.5), (0.75, 0.25));
}

#[test]
fn extra_channels_are_ignored_and_an_empty_frame_is_silence() {
    assert_eq!(mix_frame(&[0.5, 0.25, 9.0, 9.0], 0.0, 0.0), (0.5, 0.25));
    assert_eq!(mix_frame(&[], 0.0, 0.0), (0.0, 0.0));
}

#[test]
fn control_clamps_out_of_range_values_and_tracks_liveness() {
    let control = VoiceControl::new(&VoiceMix {
        gain: -1.0,
        pan: -3.0,
        spatial_blend: 2.0,
        speed: -1.0,
        paused: false,
    });
    assert_eq!(control.gain.get(), 0.0);
    assert_eq!(control.pan.get(), -1.0);
    assert_eq!(control.spatial_blend.get(), 1.0);
    assert_eq!(control.speed.get(), 0.0);
    assert!(control.is_live());
    control.stop();
    assert!(!control.is_live());
}
