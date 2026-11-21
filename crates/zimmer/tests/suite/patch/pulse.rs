//! Pulse width: a square narrowed into the chiptune duties, and an LFO that
//! moves it (#1773).
//!
//! What the document says and what a rendered note does with it — as against
//! `core::osc::pulse`, where the waveform itself is pinned.

use crate::common::{minimal, opts, osc, refusal, render, spectrum::band_power};
use scorsese_zimmer::patch::{Lfo, LfoTarget, Osc, Source, Wave};
use scorsese_zimmer::{Patch, SynthError};

/// A single oscillator of `wave` at `pulse_width`.
fn pulse(wave: Wave, pulse_width: f32) -> Patch {
    minimal(Source::OscStack {
        oscs: vec![Osc {
            pulse_width,
            ..osc(wave, 0.0, 0)
        }],
    })
}

/// The same patch with an LFO moving the width by `depth`.
fn modulated(patch: Patch, depth: f32) -> Patch {
    Patch {
        lfo: Some(Lfo::new(2.0, depth, LfoTarget::PulseWidth)),
        ..patch
    }
}

/// A3, 220 Hz, held half a second: long enough for two analysis blocks.
fn note(patch: &Patch) -> Vec<f32> {
    render(patch, 57.0, &opts(0.5))
}

/// The default is the square there always was, and it is not written down —
/// so a recipe from before the field keeps its bytes and its cached bake.
#[test]
fn a_half_width_square_is_written_without_it() {
    let json = pulse(Wave::Square, 0.5).to_json().expect("serialise");
    assert!(!json.contains("pulse_width"), "the default is in {json}");
    let narrow = pulse(Wave::Square, 0.25);
    let json = narrow.to_json().expect("serialise");
    assert!(json.contains("\"pulse_width\": 0.25"), "{json}");
    assert_eq!(Patch::from_json(&json).expect("deserialise"), narrow);
}

/// The audible difference, by the number: a 50% square has only odd
/// harmonics, and a 25% pulse has its second back — which is why the three
/// duties are three instruments and not one at three levels.
#[test]
fn narrowing_a_square_brings_its_even_harmonics_in() {
    let second = |patch: &Patch| {
        let buf = note(patch);
        band_power(&buf, 440.0) / band_power(&buf, 220.0)
    };
    let square = second(&pulse(Wave::Square, 0.5));
    let quarter = second(&pulse(Wave::Square, 0.25));
    assert!(
        square < 0.01,
        "a square carries a second harmonic: {square}"
    );
    assert!(
        quarter > 0.2,
        "a 25% pulse has no second harmonic: {quarter}"
    );
}

/// Past either end is the end: a pulse of zero width is silence, which is
/// never what was meant, so the width is clamped rather than refused.
#[test]
fn a_width_past_the_range_is_clamped_to_its_edge() {
    assert_eq!(
        note(&pulse(Wave::Square, 0.0)),
        note(&pulse(Wave::Square, 0.05))
    );
    assert_eq!(
        note(&pulse(Wave::Square, 1.0)),
        note(&pulse(Wave::Square, 0.95))
    );
}

/// A width on a wave that has none is a field nothing can hear.
#[test]
fn a_pulse_width_on_another_wave_is_refused() {
    for wave in [Wave::Sine, Wave::Triangle, Wave::Saw] {
        let refused = refusal(&pulse(wave, 0.25), 57.0, &opts(0.5));
        assert_eq!(refused, SynthError::PulseWidthOffSquare { wave });
        let said = refused.to_string();
        let name = format!("`{}`", format!("{wave:?}").to_lowercase());
        assert!(
            said.contains("`pulse_width`") && said.contains(&name),
            "{said}"
        );
    }
}

/// An LFO aimed at the width needs a square to reach, and so does nothing on a
/// stack of saws or on any other source.
#[test]
fn a_pulse_width_lfo_with_no_square_to_move_is_refused() {
    let saws = modulated(pulse(Wave::Saw, 0.5), 0.3);
    let refused = refusal(&saws, 57.0, &opts(0.5));
    assert_eq!(refused, SynthError::NoPulseToModulate);
    assert!(refused.to_string().contains("no `square`"), "{refused}");
    let pluck = modulated(
        minimal(Source::Karplus {
            damping: 0.996,
            brightness: 0.5,
        }),
        0.3,
    );
    assert_eq!(
        refusal(&pluck, 57.0, &opts(0.5)),
        SynthError::NoPulseToModulate
    );
}

/// PWM through the whole path: the moving width changes the note, a still one
/// is the square it sits on, and one square in a mixed stack is enough.
#[test]
fn an_lfo_moves_the_width_of_every_square_in_the_stack() {
    let square = pulse(Wave::Square, 0.5);
    assert_eq!(note(&modulated(square.clone(), 0.0)), note(&square));
    assert_ne!(note(&modulated(square.clone(), 0.3)), note(&square));
    let mixed = minimal(Source::OscStack {
        oscs: vec![osc(Wave::Saw, 0.0, 0), osc(Wave::Square, 0.0, -1)],
    });
    assert_ne!(note(&modulated(mixed.clone(), 0.3)), note(&mixed));
}
