//! The additive source through the whole path: what a series is refused for, what an omitted field
//! means, and how a partial's own decay meets the amp envelope above it.

use crate::common::{adsr, minimal, opts, peak, render};
use rusty_zimmer::patch::{Partial, Source, MAX_PARTIALS};
use rusty_zimmer::{Patch, SynthError};

fn partial(ratio: f32, gain: f32, decay: f32) -> Partial {
    Partial {
        ratio,
        gain,
        detune_cents: 0.0,
        decay,
    }
}

fn series(partials: Vec<Partial>) -> Source {
    Source::Additive { partials }
}

#[test]
fn a_plain_harmonic_series_is_playable() {
    minimal(series(vec![partial(1.0, 1.0, 0.0), partial(2.0, 0.5, 0.4)]))
        .validate()
        .expect("a legal patch");
}

#[test]
fn a_series_with_nothing_in_it_is_refused() {
    assert_eq!(
        minimal(series(vec![])).validate(),
        Err(SynthError::EmptyPartials)
    );
}

#[test]
fn an_oversized_series_says_what_the_limit_is() {
    let partials = vec![partial(1.0, 1.0, 0.0); MAX_PARTIALS + 1];
    let found = partials.len();
    assert_eq!(
        minimal(series(partials)).validate(),
        Err(SynthError::TooManyPartials {
            found,
            limit: MAX_PARTIALS,
        })
    );
    minimal(series(vec![partial(1.0, 1.0, 0.0); MAX_PARTIALS]))
        .validate()
        .expect("the cap itself is legal");
}

#[test]
fn a_partial_at_or_below_dc_is_refused_by_index() {
    for (index, bad) in [(0, 0.0), (2, -3.0), (1, f32::INFINITY)] {
        let mut partials = vec![partial(1.0, 1.0, 0.0); 3];
        partials[index].ratio = bad;
        assert_eq!(
            minimal(series(partials)).validate(),
            Err(SynthError::BadPartialRatio { index, ratio: bad })
        );
    }
}

#[test]
fn a_series_weighted_entirely_to_zero_is_refused() {
    assert_eq!(
        minimal(series(vec![
            partial(1.0, 0.0, 0.0),
            partial(2.0, -1.0, 0.0)
        ]))
        .validate(),
        Err(SynthError::SilentPartials)
    );
}

#[test]
fn a_partial_is_a_ratio_and_three_defaults() {
    let json = r#"{
        "source": { "kind": "additive", "partials": [{ "ratio": 1 }, { "ratio": 3 }] },
        "amp": { "a": 0.01, "d": 0.0, "s": 1.0, "r": 0.1 }
    }"#;
    let patch = Patch::from_json(json).expect("it parses");
    assert_eq!(
        patch.source,
        series(vec![partial(1.0, 1.0, 0.0), partial(3.0, 1.0, 0.0)])
    );
    let round_tripped = patch.to_json().expect("serialise");
    assert_eq!(Patch::from_json(&round_tripped).expect("re-read"), patch);
}

#[test]
fn the_tag_is_the_word_the_survey_counts() {
    let patch = minimal(series(vec![partial(1.0, 1.0, 0.0)]));
    assert_eq!(patch.source.kind(), "additive");
    assert!(
        patch.to_json().expect("serialise").contains("\"additive\""),
        "the tag has to survive serialisation"
    );
}

#[test]
fn a_partial_decay_and_the_amp_envelope_multiply() {
    let long = partial(1.0, 1.0, 2.0);
    let held = {
        let mut patch = minimal(series(vec![long]));
        patch.amp = adsr(0.0, 0.0, 1.0, 0.0);
        patch
    };
    let clipped = {
        let mut patch = held.clone();
        patch.amp = adsr(0.0, 0.05, 0.0, 0.0);
        patch
    };
    let at_200ms = |patch: &Patch| peak(&render(patch, 45.0, &opts(1.0))[8_820..9_261]);
    assert!(at_200ms(&held) > 0.85, "a slow decay is still nearly full");
    assert!(
        at_200ms(&clipped) < 0.01,
        "the short envelope took it first"
    );

    let mut brief = held.clone();
    brief.source = series(vec![partial(1.0, 1.0, 0.02)]);
    assert!(at_200ms(&brief) < 0.01, "the partial went on its own");
}
