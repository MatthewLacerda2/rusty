use rodio::buffer::SamplesBuffer;

use super::*;

const RATE: u32 = 48_000;

/// The steady-state output peak of a constant-level stereo signal through `mode`.
fn settled_peak(mode: SpeakerMode, level: f32) -> f32 {
    let mut comp = Compressor::new(RATE);
    let mut out = 0.0;
    for _ in 0..RATE {
        out = process_frame(mode, &mut comp, level, level).0;
    }
    out.abs()
}

#[test]
fn home_theater_is_bit_identical() {
    let mut comp = Compressor::new(RATE);
    for x in [0.0, 1e-7, -0.25, 0.999, 1.5, -3.0] {
        let (l, r) = process_frame(SpeakerMode::HomeTheater, &mut comp, x, -x);
        assert_eq!((l.to_bits(), r.to_bits()), (x.to_bits(), (-x).to_bits()));
    }
}

#[test]
fn tv_narrows_the_dynamic_range() {
    let (quiet, loud) = (0.01, 0.5);
    let reference = settled_peak(SpeakerMode::HomeTheater, quiet)
        / settled_peak(SpeakerMode::HomeTheater, loud);
    let tv = settled_peak(SpeakerMode::Tv, quiet) / settled_peak(SpeakerMode::Tv, loud);
    assert!(tv > reference * 2.0, "tv {tv} vs reference {reference}");
    assert!(
        settled_peak(SpeakerMode::Tv, quiet) > quiet,
        "quiet cue lifted"
    );
}

#[test]
fn tv_never_clips() {
    for level in [0.5, 1.0, 4.0] {
        let peak = settled_peak(SpeakerMode::Tv, level);
        assert!(peak < 1.0, "level {level} -> {peak}");
    }
    assert!(limit(100.0) < 1.0 && limit(-100.0) > -1.0);
    assert_eq!(limit(0.5), 0.5);
}

#[test]
fn headphones_crossfeed_keeps_a_hard_left_signal_out_of_one_ear_only() {
    let mut comp = Compressor::new(RATE);
    let (l, r) = process_frame(SpeakerMode::Headphones, &mut comp, 1.0, 0.0);
    assert!(r > 0.0 && l > r);
    let (l, r) = process_frame(SpeakerMode::Headphones, &mut comp, 0.3, 0.3);
    assert!(
        (l - 0.3).abs() < 1e-6 && (r - 0.3).abs() < 1e-6,
        "centre kept"
    );
}

#[test]
fn control_round_trips_every_mode() {
    let control = MasterControl::new(SpeakerMode::default());
    assert_eq!(control.get(), SpeakerMode::HomeTheater);
    for mode in SpeakerMode::ALL {
        control.set(mode);
        assert_eq!(control.get(), mode);
    }
}

#[test]
fn bus_passes_frames_in_order_and_plays_silence_once_drained() {
    let control = MasterControl::new(SpeakerMode::HomeTheater);
    let input = SamplesBuffer::new(2, RATE, vec![0.1, 0.2, 0.3, 0.4]);
    let bus = MasterBus::new(input, control);
    assert_eq!(bus.channels(), 2);
    let out: Vec<f32> = bus.take(6).collect();
    assert_eq!(out, vec![0.1, 0.2, 0.3, 0.4, 0.0, 0.0]);
}
