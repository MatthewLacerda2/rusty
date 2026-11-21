//! That each optional stage reaches the signal — asked about the samples,
//! never about whether the code ran.

use crate::common::{adsr, brightness, opts, peak, render, rising_crossings, saw_patch};
use scorsese_zimmer::patch::{Adsr, Filter, FilterKind, Lfo, LfoTarget, Slope};

/// A lowpass at `cutoff` with no envelope movement.
fn lowpass(cutoff: f32, resonance: f32) -> Filter {
    Filter {
        kind: FilterKind::Lowpass,
        slope: Slope::Db12,
        cutoff,
        resonance,
        env_octaves: 0.0,
        vel_octaves: 0.0,
        key_track: 0.0,
        adsr: Adsr::default(),
    }
}

#[test]
fn the_filter_stage_removes_harmonics() {
    let mut filtered = saw_patch();
    filtered.filter = Some(lowpass(300.0, 0.0));
    let open = brightness(&render(&saw_patch(), 60.0, &opts(0.5)));
    let closed = brightness(&render(&filtered, 60.0, &opts(0.5)));
    assert!(
        closed < open * 0.5,
        "a 300 Hz lowpass must dull a saw: {closed} vs {open}"
    );
}

#[test]
fn the_filter_envelope_sweeps_the_cutoff_over_the_note() {
    let mut patch = saw_patch();
    patch.filter = Some(Filter {
        env_octaves: 5.0,
        adsr: adsr(0.0, 0.4, 0.0, 0.0),
        ..lowpass(200.0, 0.0)
    });
    let buf = render(&patch, 60.0, &opts(0.5));
    let bright = brightness(&buf[..4410]);
    let dull = brightness(&buf[17_640..22_050]);
    assert!(bright > dull * 2.0, "the sweep closes: {bright} → {dull}");
}

#[test]
fn a_pitch_lfo_bends_the_note() {
    let mut vibrato = saw_patch();
    vibrato.lfo = Some(Lfo::new(6.0, 2.0, LfoTarget::Pitch));
    let plain = render(&saw_patch(), 69.0, &opts(1.0));
    let bent = render(&vibrato, 69.0, &opts(1.0));
    assert_ne!(
        rising_crossings(&plain),
        rising_crossings(&bent),
        "vibrato moves the pitch"
    );
}

/// Three quarters of a cycle at 6 Hz: `44_100 * 3 / (4 * 6)`, where the wave
/// is at its trough and a full-depth tremolo is holding the note at silence.
const TROUGH: usize = 5_512;

/// A window around it, a couple of saw periods wide, so the assertion is about
/// the gain over a stretch rather than about one sample that might have been
/// near a zero crossing anyway.
const AROUND: usize = 60;

/// The distinction the LFO targets exist to make: tremolo is a level move, so
/// it must leave the pitch alone entirely.
#[test]
fn a_tremolo_lfo_touches_level_and_not_pitch() {
    let mut patch = saw_patch();
    patch.lfo = Some(Lfo::new(6.0, 1.0, LfoTarget::Amp));
    let plain = render(&saw_patch(), 69.0, &opts(1.0));
    let tremolo = render(&patch, 69.0, &opts(1.0));
    assert_eq!(
        rising_crossings(&plain),
        rising_crossings(&tremolo),
        "tremolo is not a pitch move"
    );
    assert!(peak(&tremolo) > 0.5, "but it is not silence either");
    // Full depth dips all the way *to* silence and never past unity, which is
    // the tremolo multiplying into the gain rather than dividing it: a divide
    // reaches infinity at the trough and the note is a click instead.
    assert!(
        tremolo.iter().all(|sample| sample.abs() <= 1.0 + 1e-3),
        "a tremolo must not make a note louder than the note"
    );
    let trough = peak(&tremolo[TROUGH - AROUND..TROUGH + AROUND]);
    assert!(trough < 0.01, "full depth must reach silence, not {trough}");
}

#[test]
fn a_cutoff_lfo_wobbles_the_filter() {
    let mut patch = saw_patch();
    patch.filter = Some(lowpass(800.0, 0.5));
    let steady = render(&patch, 45.0, &opts(1.0));
    patch.lfo = Some(Lfo::new(3.0, 2.0, LfoTarget::Cutoff));
    let wobbled = render(&patch, 45.0, &opts(1.0));
    assert_ne!(steady, wobbled);
    assert!(
        wobbled.iter().all(|sample| sample.is_finite()),
        "a resonant filter swept hard must stay stable"
    );
}
