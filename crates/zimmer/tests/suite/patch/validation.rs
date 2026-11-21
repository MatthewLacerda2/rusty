//! What a patch is refused for — and, as much as it matters, what it is not.
//!
//! Only the settings that would produce silence, a divide-by-zero or an
//! unstable filter are rejected. Everything else renders, however ugly.

use crate::common::{minimal, noise, osc};
use scorsese_zimmer::SynthError;
use scorsese_zimmer::patch::{
    Adsr, Filter, FilterKind, Lfo, LfoTarget, MAX_OSCS, Partial, Slope, Source, Wave,
};

#[test]
fn every_source_kind_is_playable() {
    for source in [
        Source::OscStack {
            oscs: vec![osc(Wave::Sine, 0.0, 0)],
        },
        Source::Karplus {
            damping: 0.99,
            brightness: 0.5,
        },
        noise(),
        Source::Fm2 {
            ratio: 2.0,
            index: 3.0,
            vel_index: 0.0,
            mod_decay: 0.3,
        },
        Source::Additive {
            partials: vec![Partial {
                ratio: 1.0,
                gain: 1.0,
                detune_cents: 0.0,
                decay: 0.0,
            }],
        },
    ] {
        minimal(source).validate().expect("a legal patch");
    }
}

#[test]
fn an_empty_stack_is_refused() {
    assert_eq!(
        minimal(Source::OscStack { oscs: vec![] }).validate(),
        Err(SynthError::EmptyOscStack)
    );
}

#[test]
fn an_oversized_stack_says_what_the_limit_is() {
    let oscs = vec![osc(Wave::Saw, 0.0, 0); MAX_OSCS + 1];
    let found = oscs.len();
    assert_eq!(
        minimal(Source::OscStack { oscs }).validate(),
        Err(SynthError::TooManyOscs {
            found,
            limit: MAX_OSCS
        })
    );
}

/// A stack of oscillators all weighted zero parses fine and renders silence,
/// which is never what was meant — so it is caught rather than delivered.
#[test]
fn a_stack_weighted_entirely_to_zero_is_refused() {
    let mut silent = osc(Wave::Saw, 0.0, 0);
    silent.gain = 0.0;
    assert_eq!(
        minimal(Source::OscStack { oscs: vec![silent] }).validate(),
        Err(SynthError::SilentOscStack)
    );
}

#[test]
fn a_zero_cutoff_is_not_a_filter() {
    let mut patch = minimal(noise());
    patch.filter = Some(Filter {
        kind: FilterKind::Lowpass,
        slope: Slope::Db12,
        cutoff: 0.0,
        resonance: 0.0,
        env_octaves: 0.0,
        vel_octaves: 0.0,
        key_track: 0.0,
        adsr: Adsr::default(),
    });
    assert_eq!(patch.validate(), Err(SynthError::BadCutoff { cutoff: 0.0 }));
}

#[test]
fn a_negative_lfo_rate_is_not_a_rate() {
    let mut patch = minimal(noise());
    patch.lfo = Some(Lfo::new(-1.0, 0.5, LfoTarget::Amp));
    assert_eq!(
        patch.validate(),
        Err(SynthError::NegativeLfoRate { rate: -1.0 })
    );
}

#[test]
fn an_fm_modulator_needs_somewhere_to_sit() {
    assert_eq!(
        minimal(Source::Fm2 {
            ratio: 0.0,
            index: 1.0,
            vel_index: 0.0,
            mod_decay: 0.3,
        })
        .validate(),
        Err(SynthError::BadFmRatio { ratio: 0.0 })
    );
}

/// The refusal carries the offending number, so an agent repairing a recipe
/// unattended can fix it from the error alone.
#[test]
fn a_refusal_names_the_value_that_caused_it() {
    let mut patch = minimal(noise());
    patch.filter = Some(Filter {
        kind: FilterKind::Highpass,
        slope: Slope::Db12,
        cutoff: -40.0,
        resonance: 0.0,
        env_octaves: 0.0,
        vel_octaves: 0.0,
        key_track: 0.0,
        adsr: Adsr::default(),
    });
    let message = patch.validate().expect_err("refused").to_string();
    assert!(message.contains("-40"), "got {message}");
}
