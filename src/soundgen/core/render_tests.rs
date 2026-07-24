//! Tests for the note renderer: the shape of the buffer it returns, and that each
//! optional stage (filter, LFO, velocity) actually reaches the signal.

use super::*;
use crate::soundgen::patch::{Adsr, Filter, FilterKind, Fx, Osc, Source, Wave};

/// A plain saw through a fully-sustaining envelope — the control case.
fn saw_patch() -> Patch {
    Patch {
        source: Source::OscStack {
            oscs: vec![Osc {
                wave: Wave::Saw,
                detune_cents: 0.0,
                gain: 1.0,
                octave: 0,
            }],
        },
        amp: Adsr {
            a: 0.0,
            d: 0.0,
            s: 1.0,
            r: 0.0,
        },
        filter: None,
        lfo: None,
        fx: vec![],
    }
}

fn opts(duration: f32) -> NoteOpts {
    NoteOpts {
        duration,
        velocity: 1.0,
        seed: 0,
    }
}

fn peak(buf: &[f32]) -> f32 {
    buf.iter().fold(0.0f32, |m, s| m.max(s.abs()))
}

/// High-frequency content: the energy of the first difference relative to the
/// signal's own energy. Amplitude-invariant (a quieter signal is not a duller one),
/// which is what makes it a fair brightness measure across a filter.
fn brightness(buf: &[f32]) -> f32 {
    let signal: f32 = buf.iter().map(|s| s * s).sum();
    let edges: f32 = buf.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum();
    if signal > 0.0 {
        edges / signal
    } else {
        0.0
    }
}

#[test]
fn the_buffer_is_the_gate_plus_the_release_tail() {
    let mut patch = saw_patch();
    patch.amp.r = 0.25;
    let buf = render(&patch, 60.0, &opts(0.5)).expect("render");
    assert_eq!(buf.len(), sample_count(0.75), "0.5 s gate + 0.25 s release");
    // The tail is audible at the start of the release and gone by its end.
    assert!(
        buf[22_100..].iter().any(|s| s.abs() > 0.1),
        "the tail rings"
    );
    assert!(buf[buf.len() - 1].abs() < 1e-3, "and ends in silence");
}

#[test]
fn an_fx_chain_lengthens_the_buffer_so_its_tail_survives() {
    let mut patch = saw_patch();
    patch.fx = vec![Fx::Reverb {
        size: 0.8,
        damp: 0.5,
        mix: 0.4,
    }];
    let dry = render(&saw_patch(), 60.0, &opts(0.2)).expect("render");
    let wet = render(&patch, 60.0, &opts(0.2)).expect("render");
    assert!(wet.len() > dry.len(), "the reverb tail needs room");
    assert!(
        wet[dry.len()..].iter().any(|s| s.abs() > 1e-4),
        "and rings there"
    );
}

#[test]
fn velocity_scales_the_note_linearly() {
    let soft = render(
        &saw_patch(),
        60.0,
        &NoteOpts {
            velocity: 0.25,
            ..opts(0.2)
        },
    )
    .expect("render");
    let loud = render(&saw_patch(), 60.0, &opts(0.2)).expect("render");
    assert!((peak(&loud) * 0.25 - peak(&soft)).abs() < 0.01);
}

#[test]
fn the_filter_stage_removes_harmonics() {
    let mut filtered = saw_patch();
    filtered.filter = Some(Filter {
        kind: FilterKind::Lowpass,
        cutoff: 300.0,
        resonance: 0.0,
        env_amount: 0.0,
        adsr: Adsr::default(),
    });
    let open = render(&saw_patch(), 60.0, &opts(0.5)).expect("render");
    let closed = render(&filtered, 60.0, &opts(0.5)).expect("render");
    assert!(
        brightness(&closed) < brightness(&open) * 0.5,
        "a 300 Hz lowpass must dull a saw: {} vs {}",
        brightness(&closed),
        brightness(&open)
    );
}

#[test]
fn the_filter_envelope_sweeps_the_cutoff_over_the_note() {
    let mut patch = saw_patch();
    patch.filter = Some(Filter {
        kind: FilterKind::Lowpass,
        cutoff: 200.0,
        resonance: 0.0,
        env_amount: 6000.0,
        adsr: Adsr {
            a: 0.0,
            d: 0.4,
            s: 0.0,
            r: 0.0,
        },
    });
    let buf = render(&patch, 60.0, &opts(0.5)).expect("render");
    let bright = brightness(&buf[..4410]);
    let dull = brightness(&buf[17_640..22_050]);
    assert!(bright > dull * 2.0, "the sweep closes: {bright} → {dull}");
}

#[test]
fn a_pitch_lfo_bends_the_note_and_a_tremolo_lfo_does_not() {
    let mut vibrato = saw_patch();
    vibrato.lfo = Some(Lfo {
        rate: 6.0,
        depth: 2.0,
        target: LfoTarget::Pitch,
    });
    let plain = render(&saw_patch(), 69.0, &opts(1.0)).expect("render");
    let bent = render(&vibrato, 69.0, &opts(1.0)).expect("render");
    let crossings = |b: &[f32]| b.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
    assert_ne!(
        crossings(&plain),
        crossings(&bent),
        "vibrato moves the pitch"
    );

    let mut tremolo_patch = saw_patch();
    tremolo_patch.lfo = Some(Lfo {
        rate: 6.0,
        depth: 1.0,
        target: LfoTarget::Amp,
    });
    let tremolo = render(&tremolo_patch, 69.0, &opts(1.0)).expect("render");
    assert_eq!(
        crossings(&plain),
        crossings(&tremolo),
        "tremolo touches level, not pitch"
    );
    assert!(peak(&tremolo) > 0.5, "but it is not silence");
}

#[test]
fn a_cutoff_lfo_wobbles_the_filter() {
    let mut patch = saw_patch();
    patch.filter = Some(Filter {
        kind: FilterKind::Lowpass,
        cutoff: 800.0,
        resonance: 0.5,
        env_amount: 0.0,
        adsr: Adsr::default(),
    });
    let steady = render(&patch, 45.0, &opts(1.0)).expect("render");
    patch.lfo = Some(Lfo {
        rate: 3.0,
        depth: 2.0,
        target: LfoTarget::Cutoff,
    });
    let wobbled = render(&patch, 45.0, &opts(1.0)).expect("render");
    assert_ne!(steady, wobbled);
    assert!(wobbled.iter().all(|s| s.is_finite()));
}

#[test]
fn an_absurd_duration_is_rejected_or_bounded_but_never_unbounded() {
    assert!(render(&saw_patch(), 60.0, &opts(0.0)).is_err());
    assert!(render(&saw_patch(), 60.0, &opts(-1.0)).is_err());
    assert!(render(&saw_patch(), 60.0, &opts(f32::NAN)).is_err());
    let long = render(&saw_patch(), 60.0, &opts(1e6)).expect("clamped, not refused");
    assert_eq!(long.len(), sample_count(MAX_SECONDS));
}

#[test]
fn an_invalid_patch_is_refused_before_any_sample_is_written() {
    let mut broken = saw_patch();
    broken.source = Source::OscStack { oscs: vec![] };
    assert!(render(&broken, 60.0, &opts(0.5)).is_err());
}
