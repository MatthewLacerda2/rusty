//! The LFO curves by the number. Expected values are worked out by hand and
//! written as literals, as [`super::super::tracks`]'s tests are, with
//! `assert_eq!` where the maths is exact in binary and a tight epsilon only
//! where an irrational lands between two floats.

use super::*;

/// **Not 1 Hz**: `TAU * rate` and `TAU / rate` agree at 1. 5 Hz divides
/// [`RATE`] evenly four ways, so the quarter-cycle index is an integer.
const RATE_HZ: f32 = 5.0;
const PEAK: usize = 2_205;
const HALF: usize = 4_410;
const TROUGH: usize = 6_615;
const CYCLE: usize = 8_820;

fn lfo(shape: LfoShape) -> Lfo {
    Lfo {
        shape,
        ..Lfo::new(RATE_HZ, 1.0, LfoTarget::Pitch)
    }
}

/// The wave of one LFO alone, struck with seed 7, `n` frames.
fn waves(lfo: &Lfo, n: usize) -> Vec<f32> {
    let lfos = Lfos::of([lfo], 7, None, n);
    (0..n).map(|i| lfos.offset(lfo.target, i)).collect()
}

#[test]
fn the_wave_starts_at_zero_whatever_the_rate() {
    for rate in [0.1, 1.0, 5.0, 10.0, 100.0] {
        let l = Lfo::new(rate, 1.0, LfoTarget::Pitch);
        assert_eq!(waves(&l, 1)[0], 0.0, "rate {rate}");
    }
}

/// The sine is the one sine it always was: `sin(TAU * rate * i / RATE)`,
/// bit for bit, which is what keeps every existing recipe's samples.
#[test]
fn a_sine_with_no_onset_is_the_wave_it_always_was() {
    let l = Lfo::new(5.5, 1.0, LfoTarget::Pitch);
    for (i, value) in waves(&l, 3000).into_iter().enumerate() {
        assert_eq!(value, (TAU * 5.5 * i as f32 / RATE).sin(), "sample {i}");
    }
}

#[test]
fn a_negative_rate_holds_the_wave_still() {
    let l = Lfo::new(-3.0, 1.0, LfoTarget::Pitch);
    assert!(waves(&l, 1000).iter().all(|value| *value == 0.0));
}

/// Each shape at the start, a quarter, a half and three quarters of its
/// cycle. The sine's peaks are `sin` of an irrational and land a few ulps
/// off one; the rest are exact.
/// The triangle's last quarter climbs back from the bottom: seven eighths in
/// it is half way up, where the falling side's line would be lower still.
#[test]
fn a_triangle_climbs_back_in_its_last_quarter() {
    let wave = waves(&lfo(LfoShape::Triangle), CYCLE);
    assert!((wave[7_717] + 0.5).abs() < 1e-3, "{}", wave[7_717]);
}

#[test]
fn each_shape_at_its_quarter_cycle_points() {
    let points = [0, PEAK, HALF, TROUGH];
    let cases = [
        (LfoShape::Sine, [0.0, 1.0, 0.0, -1.0]),
        (LfoShape::Triangle, [0.0, 1.0, 0.0, -1.0]),
        (LfoShape::SawUp, [0.0, 0.5, -1.0, -0.5]),
        (LfoShape::SawDown, [0.0, -0.5, 1.0, 0.5]),
        (LfoShape::Square, [1.0, 1.0, -1.0, -1.0]),
    ];
    for (shape, expected) in cases {
        let wave = waves(&lfo(shape), CYCLE);
        for (at, want) in points.into_iter().zip(expected) {
            assert!(
                (wave[at] - want).abs() < 1e-3,
                "{shape:?} at {at}: {} vs {want}",
                wave[at]
            );
        }
    }
}

/// Sample-and-hold: one value held through a cycle, a new one the next, and
/// the same ones for the same seed.
#[test]
fn a_random_lfo_holds_one_draw_per_cycle() {
    let l = lfo(LfoShape::Random);
    let wave = waves(&l, 3 * CYCLE);
    let first = wave[1];
    assert!(wave[1..CYCLE - 1].iter().all(|value| *value == first));
    assert_ne!(wave[CYCLE + 1], first, "the next cycle draws again");
    assert!(wave.iter().all(|value| (-1.0..=1.0).contains(value)));
    // Drawn from the whole range, both signs, over enough cycles.
    let many = waves(&l, 40 * CYCLE);
    assert!(many.iter().any(|value| *value < -0.2) && many.iter().any(|value| *value > 0.2));
    assert_eq!(wave, waves(&l, 3 * CYCLE), "deterministic");
    let other = Lfos::of([&l], 8, None, 2);
    assert_ne!(
        other.offset(LfoTarget::Pitch, 1),
        first,
        "the seed moves it"
    );
}

/// Nothing through the delay, growing over the fade, all of it after. Read
/// on the amp, whose dip is the depth wherever a square sits at its bottom
/// (−1), which is its second half.
#[test]
fn depth_is_nothing_through_the_delay_and_full_after_the_fade() {
    let l = Lfo {
        shape: LfoShape::Square,
        delay: 0.5,
        fade: 1.0,
        ..Lfo::new(RATE_HZ, 1.0, LfoTarget::Amp)
    };
    let n = 2 * 44_100;
    let lfos = Lfos::of([&l], 0, None, n);
    for i in [0, 10_000, 22_049] {
        assert_eq!(lfos.tremolo(i), 1.0, "still at {i}");
    }
    // The cycle restarts when the delay ends, so three quarters of a cycle
    // later the square is at its bottom, with the depth 6 615 / 44 100 of
    // the way up the fade: a dip of 0.15.
    let early = 22_050 + TROUGH;
    assert!(
        (lfos.tremolo(early) - 0.85).abs() < 1e-6,
        "{}",
        lfos.tremolo(early)
    );
    let after = 22_050 + 44_100 + TROUGH;
    assert_eq!(lfos.tremolo(after), 0.0);
}

/// A clock whose tempo doubles at beat 4: 120 bpm, then 240.
struct Doubling;

impl Grid for Doubling {
    fn beat_at(&self, frame: usize) -> f32 {
        let seconds = frame as f32 / RATE;
        if seconds < 2.0 {
            seconds * 2.0
        } else {
            4.0 + (seconds - 2.0) * 4.0
        }
    }
}

/// One cycle per half beat whatever the tempo: before the change a cycle is
/// a quarter of a second, after it an eighth, and the phase comes from the
/// song, not from the note — a note struck off the grid starts mid-cycle.
#[test]
fn a_beats_lfo_completes_one_cycle_per_its_beats_under_a_tempo_change() {
    let l = Lfo {
        beats: Some(0.5),
        ..Lfo::new(0.0, 1.0, LfoTarget::Pitch)
    };
    let onset = Onset {
        grid: &Doubling,
        frame: 0,
    };
    let n = 4 * 44_100;
    let lfos = Lfos::of([&l], 0, Some(&onset), n);
    let at = |seconds: f32| lfos.offset(LfoTarget::Pitch, (seconds * RATE) as usize);
    // An eighth of a beat at 120 bpm is a sixteenth of a second: the top.
    assert!((at(0.0625) - 1.0).abs() < 1e-3);
    assert!((at(0.1875) + 1.0).abs() < 1e-3);
    // After the change it is a thirty-second of a second.
    assert!((at(2.0 + 0.03125) - 1.0).abs() < 1e-3);
    assert!((at(2.0 + 0.09375) + 1.0).abs() < 1e-3);
    // Struck a sixteenth of a second (an eighth of a beat) into the song,
    // the note's first sample is already at the top.
    let late = Onset {
        grid: &Doubling,
        frame: 2_756,
    };
    let lfos = Lfos::of([&l], 0, Some(&late), 1);
    assert!((lfos.offset(LfoTarget::Pitch, 0) - 1.0).abs() < 2e-3);
}

/// Two LFOs on one target are the sum of each alone; two on the amp are the
/// product of their dips.
#[test]
fn two_lfos_on_one_target_add_and_two_dips_multiply() {
    let slow = Lfo::new(RATE_HZ, 2.0, LfoTarget::Cutoff);
    let fast = Lfo {
        shape: LfoShape::Square,
        ..Lfo::new(3.0 * RATE_HZ, 0.5, LfoTarget::Cutoff)
    };
    let both = Lfos::of([&slow, &fast], 0, None, CYCLE);
    let (a, b) = (waves(&slow, CYCLE), waves(&fast, CYCLE));
    for i in [0, 700, PEAK, HALF, TROUGH] {
        assert_eq!(both.offset(LfoTarget::Cutoff, i), a[i] + b[i], "sample {i}");
    }
    let dip = |depth| Lfo {
        shape: LfoShape::Square,
        ..Lfo::new(RATE_HZ, depth, LfoTarget::Amp)
    };
    let dips = Lfos::of([&dip(0.5), &dip(0.5)], 0, None, CYCLE);
    // Each square sits at its bottom in its second half: 0.5 of the level,
    // and two of them a quarter.
    assert_eq!(dips.tremolo(TROUGH), 0.25);
    assert_eq!(dips.tremolo(PEAK), 1.0);
}

#[test]
fn an_lfo_aimed_elsewhere_moves_nothing_here() {
    let lfos = Lfos::of([&Lfo::new(RATE_HZ, 1.0, LfoTarget::Pitch)], 0, None, CYCLE);
    assert!(!lfos.aims_at(LfoTarget::Amp));
    assert_eq!(lfos.tremolo(TROUGH), 1.0);
    assert_eq!(lfos.offset(LfoTarget::Cutoff, PEAK), 0.0);
}

/// A full-depth dip is half way down where the wave is zero, untouched at its
/// peak and silent at its trough — the whole of `1 − depth × (1 − wave) / 2`,
/// in exact numbers.
#[test]
fn a_full_depth_tremolo_swings_from_silence_to_the_note() {
    let amp = |depth| Lfos::of([&Lfo::new(RATE_HZ, depth, LfoTarget::Amp)], 0, None, CYCLE);
    assert_eq!(amp(1.0).tremolo(0), 0.5, "the wave starts halfway");
    assert_eq!(amp(1.0).tremolo(PEAK), 1.0, "and reaches the level written");
    assert_eq!(amp(1.0).tremolo(TROUGH), 0.0, "and dips to nothing");
    assert_eq!(
        amp(0.5).tremolo(TROUGH),
        0.5,
        "half the depth, half the dip"
    );
}
