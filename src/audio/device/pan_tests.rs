//! Tests for the stereo panner (#412): the stereo rule and live retuning.

use rodio::buffer::SamplesBuffer;
use rodio::Source;

use super::*;

fn pan_all(channels: u16, samples: Vec<f32>, pan: f32, blend: f32) -> Vec<f32> {
    let src = SamplesBuffer::new(channels, 48_000, samples);
    PanSource::new(src, PanControl::new(pan, blend)).collect()
}

#[test]
fn mono_panned_hard_left_silences_the_right_channel() {
    let out = pan_all(1, vec![0.5, -0.25], -1.0, 1.0);
    assert_eq!(out, vec![0.5, 0.0, -0.25, 0.0]);
}

#[test]
fn centred_mono_plays_at_unity_on_both_channels() {
    let out = pan_all(1, vec![0.5], 0.0, 1.0);
    assert_eq!(out, vec![0.5, 0.5]);
}

#[test]
fn half_pan_right_fades_the_left_channel_linearly() {
    let out = pan_all(1, vec![1.0], 0.5, 1.0);
    assert_eq!(out, vec![0.5, 1.0]);
}

#[test]
fn fully_spatial_stereo_is_downmixed_before_panning() {
    // l = 1, r = 0 → mono 0.5 on both channels when centred.
    let out = pan_all(2, vec![1.0, 0.0], 0.0, 1.0);
    assert_eq!(out, vec![0.5, 0.5]);
}

#[test]
fn pure_2d_stereo_keeps_its_image_untouched() {
    let out = pan_all(2, vec![1.0, 0.0, 0.25, -0.5], 0.0, 0.0);
    assert_eq!(out, vec![1.0, 0.0, 0.25, -0.5]);
}

#[test]
fn half_blend_stereo_lerps_toward_the_downmix() {
    // l = 1, r = 0, mono 0.5 → halfway: (0.75, 0.25).
    let out = pan_all(2, vec![1.0, 0.0], 0.0, 0.5);
    assert_eq!(out, vec![0.75, 0.25]);
}

#[test]
fn retuning_the_control_applies_mid_stream_without_restarting() {
    let control = PanControl::new(0.0, 1.0);
    let src = SamplesBuffer::new(1, 48_000, vec![1.0_f32, 1.0]);
    let mut pan = PanSource::new(src, Arc::clone(&control));
    assert_eq!((pan.next(), pan.next()), (Some(1.0), Some(1.0)));
    control.set(1.0, 1.0);
    assert_eq!((pan.next(), pan.next()), (Some(0.0), Some(1.0)));
    assert_eq!(pan.next(), None);
}

#[test]
fn output_is_always_stereo_at_the_input_rate() {
    let src = SamplesBuffer::new(1, 22_050, vec![0.0_f32; 4]);
    let pan = PanSource::new(src, PanControl::new(0.0, 0.0));
    assert_eq!(pan.channels(), 2);
    assert_eq!(pan.sample_rate(), 22_050);
}

#[test]
fn control_clamps_out_of_range_values() {
    let c = PanControl::new(-3.0, 2.0);
    assert_eq!(c.load(), (-1.0, 1.0));
}
