//! Where a dropout starts and how deep it reads: gaps built at a named depth,
//! after sound of a named level, a named distance from the end.

use super::{RATE, found, steady, tone};

/// A second of a 440 Hz tone at 0.4 with 10 ms from half a second gained down
/// by `depth_db`.
fn gapped(depth_db: f64) -> Vec<f32> {
    tone(440.0, 0.4, 1.0, |t| {
        if (0.5..0.51).contains(&t) {
            -depth_db
        } else {
            0.0
        }
    })
}

/// A dropout's size is how far the gap falls under the sound either side of
/// it, measured from the energy of each.
#[test]
fn a_dropout_reads_as_deep_as_it_falls() {
    let defects = found(1, &gapped(50.0));
    assert_eq!(defects.dropouts.count, 1, "{defects:?}");
    let gap = &defects.dropouts.worst[0];
    assert!((gap.size_db - 50.0).abs() < 0.5, "{gap:?}");
}

/// The 20 ms before a gap must be −40 dBFS or louder, however loud what
/// came before them or what follows: a gap in a whisper is not a dropout
/// anybody hears.
#[test]
fn a_gap_after_quiet_sound_is_not_a_dropout() {
    let mut signal = tone(440.0, 0.4, 1.0, |t| {
        if (0.4..0.5).contains(&t) { -40.0 } else { 0.0 }
    });
    signal[RATE as usize / 2..RATE as usize / 2 + 480].fill(0.0);
    let defects = found(1, &signal);
    assert_eq!(defects.dropouts.count, 0, "{defects:?}");
}

/// A gap falls 40 dB under the sound on *both* sides: one 45 dB under the
/// sound before it and 20 under the quieter sound after is not a dropout.
#[test]
fn a_gap_falls_far_under_the_sound_on_both_sides() {
    let signal = tone(440.0, 0.4, 1.0, |t| match t {
        t if t < 0.5 => 0.0,
        t if t < 0.51 => -45.0,
        _ => -25.0,
    });
    let defects = found(1, &signal);
    assert_eq!(defects.dropouts.count, 0, "{defects:?}");
}

/// A gap needs 20 ms of sound after it to be told from the end of the sound:
/// one ending 15 ms before the end of the file is not judged.
#[test]
fn a_gap_just_before_the_end_is_not_judged() {
    let mut signal = tone(440.0, 0.4, 1.0, steady);
    let end = signal.len();
    signal[end - 1_200..end - 720].fill(0.0);
    let defects = found(1, &signal);
    assert_eq!(defects.dropouts.count, 0, "{defects:?}");
}
