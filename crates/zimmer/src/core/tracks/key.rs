//! Key tracking by the number: how far the cutoff follows the played note.
//!
//! Every expected value is a power of two worked out by hand, so each
//! assertion is exact, for the reason the parent module's tests give.

use super::cutoff_track;
use crate::core::lfo::Lfos;
use crate::patch::{Adsr, Filter, FilterKind, Slope};

/// A cutoff with no movement of its own, so the note is all that moves it.
fn lowpass(key_track: f32) -> Filter {
    Filter {
        kind: FilterKind::Lowpass,
        cutoff: 1_000.0,
        slope: Slope::Db12,
        resonance: 0.0,
        env_octaves: 0.0,
        vel_octaves: 0.0,
        key_track,
        adsr: Adsr::default(),
    }
}

/// Where the cutoff sits for a note at `midi`.
fn at(key_track: f32, midi: f32) -> f32 {
    cutoff_track(
        &lowpass(key_track),
        &Lfos::of([], 0, None, 4),
        0.5,
        4,
        1.0,
        midi,
        None,
    )[3]
}

#[test]
fn at_full_tracking_an_octave_up_doubles_the_cutoff() {
    assert_eq!(at(1.0, 72.0), 2_000.0);
    assert_eq!(at(1.0, 48.0), 500.0);
}

#[test]
fn middle_c_gets_the_written_cutoff() {
    for amount in [0.0, 0.5, 1.0] {
        assert_eq!(at(amount, 60.0), 1_000.0, "key_track {amount}");
    }
}

#[test]
fn without_tracking_the_note_moves_nothing() {
    assert_eq!(at(0.0, 84.0), 1_000.0);
    assert_eq!(at(0.0, 24.0), 1_000.0);
}

#[test]
fn half_tracking_moves_half_an_octave_per_octave() {
    assert_eq!(at(0.5, 84.0), 2_000.0);
}

/// Out of `0..=1` is clamped rather than refused.
#[test]
fn an_amount_out_of_range_is_clamped() {
    assert_eq!(at(3.0, 72.0), 2_000.0);
    assert_eq!(at(-1.0, 72.0), 1_000.0);
}

/// It joins the octave sum, so it multiplies a song's cutoff curve too.
#[test]
fn it_follows_a_moved_base_cutoff() {
    let base = [400.0; 4];
    let track = cutoff_track(
        &lowpass(1.0),
        &Lfos::of([], 0, None, 4),
        0.5,
        4,
        1.0,
        72.0,
        Some(&base),
    );
    assert_eq!(track[3], 800.0);
}
