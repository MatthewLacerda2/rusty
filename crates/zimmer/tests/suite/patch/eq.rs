//! The EQ as a *document*: how a band is written down, what an omitted field
//! means, and how many bands is too many.

use crate::common::{minimal, noise};
use scorsese_zimmer::Patch;
use scorsese_zimmer::SynthError;
use scorsese_zimmer::patch::{EqBand, EqKind, Fx, MAX_EQ_BANDS};

/// A band with everything spelled out.
fn band(kind: EqKind, freq: f32, gain_db: f32, q: f32) -> EqBand {
    EqBand {
        kind,
        freq: Some(freq),
        gain_db,
        q,
    }
}

/// A patch carrying one EQ of `bands`.
fn with_bands(bands: Vec<EqBand>) -> Patch {
    Patch {
        fx: vec![Fx::Eq { bands }],
        ..minimal(noise())
    }
}

#[test]
fn an_eq_round_trips_through_json_with_every_kind_in_it() {
    let patch = with_bands(vec![
        band(EqKind::HighPass, 80.0, 0.0, 0.707),
        band(EqKind::LowShelf, 250.0, -3.0, 0.707),
        band(EqKind::Peak, 400.0, -6.0, 2.5),
        band(EqKind::HighShelf, 4000.0, 2.0, 0.707),
        band(EqKind::LowPass, 12_000.0, 0.0, 0.707),
    ]);
    let json = patch.to_json().expect("serialise");
    assert_eq!(Patch::from_json(&json).expect("deserialise"), patch);
}

/// The tags are the words a recipe is written in, so they are the contract
/// rather than a detail of the derive — and a band is spelled the way a patch
/// filter is, one word with no underscore, which is the whole of #456.
#[test]
fn tags_are_one_word_and_named_as_documented() {
    let json = serde_json::to_string(&Fx::Eq {
        bands: vec![band(EqKind::HighPass, 250.0, 0.0, 0.707)],
    })
    .expect("serialise");
    assert!(json.contains(r#""fx":"eq""#), "got {json}");
    assert!(json.contains(r#""kind":"highpass""#), "got {json}");

    for (kind, spelling) in [
        (EqKind::LowShelf, "lowshelf"),
        (EqKind::Peak, "peak"),
        (EqKind::HighShelf, "highshelf"),
        (EqKind::LowPass, "lowpass"),
    ] {
        let json = serde_json::to_string(&kind).expect("serialise");
        assert_eq!(json, format!("\"{spelling}\""));
    }
}

/// The underscored spellings are gone rather than accepted alongside the new
/// ones. A parser that took both would keep the lookup this change exists to
/// remove, and the refusal is what makes a stale recipe loud.
#[test]
fn the_underscored_spellings_are_refused() {
    for stale in ["high_pass", "low_shelf", "high_shelf", "low_pass"] {
        let json = format!(r#"{{ "fx": "eq", "bands": [ {{ "kind": "{stale}" }} ] }}"#);
        serde_json::from_str::<Fx>(&json).expect_err(stale);
    }
}

/// The load-bearing ergonomic detail of the whole feature: a reader goes from
/// `low 61%` in a bake report to a band that treats it without converting
/// anything, because the band's own default *is* the report's boundary.
#[test]
fn an_omitted_frequency_is_the_bake_report_s_own_crossover() {
    let json = r#"{ "fx": "eq", "bands": [
        { "kind": "highpass" },
        { "kind": "lowshelf", "gain_db": -3 },
        { "kind": "peak", "gain_db": -6 },
        { "kind": "highshelf", "gain_db": 2 },
        { "kind": "lowpass" }
    ] }"#;
    let Fx::Eq { bands } = serde_json::from_str(json).expect("parses without a frequency") else {
        panic!("that is an eq");
    };
    let hz: Vec<f32> = bands.iter().map(EqBand::hz).collect();
    assert_eq!(hz, [250.0, 250.0, 250.0, 4000.0, 4000.0], "the two edges");
    assert!(
        bands.iter().all(|b| b.freq.is_none()),
        "and none was written"
    );
    assert_eq!(
        bands[0].q, 0.707,
        "a gentle, non-resonant corner by default"
    );
    assert_eq!(
        bands[0].gain_db, 0.0,
        "and a pass filter has no gain to ask"
    );
}

/// An absent `freq` stays absent when the document is saved again. A bake is
/// addressed by the hash of its recipe's *bytes*, so a serialiser that filled
/// the default in would invalidate every cached bake without changing a sample.
#[test]
fn a_defaulted_frequency_is_not_written_back() {
    let saved = serde_json::to_string(&Fx::Eq {
        bands: vec![EqBand {
            kind: EqKind::Peak,
            freq: None,
            gain_db: -6.0,
            q: 2.0,
        }],
    })
    .expect("serialise");
    assert!(!saved.contains("freq"), "freq written back: {saved}");
}

#[test]
fn a_stack_of_bands_past_the_cap_is_refused() {
    let one = band(EqKind::Peak, 250.0, -3.0, 1.0);
    with_bands(vec![one; MAX_EQ_BANDS])
        .validate()
        .expect("the cap itself is legal");
    assert_eq!(
        with_bands(vec![one; MAX_EQ_BANDS + 1]).validate(),
        Err(SynthError::TooManyEqBands {
            found: MAX_EQ_BANDS + 1,
            limit: MAX_EQ_BANDS,
        })
    );
}

/// The refusal reaches a note being rendered rather than only a document being
/// checked, and it says the number — an agent repairing a recipe unattended has
/// to be able to act on the message without a second look at the docs.
#[test]
fn the_refusal_reaches_a_note_and_names_the_cap() {
    let over = with_bands(vec![band(EqKind::Peak, 250.0, -3.0, 1.0); MAX_EQ_BANDS + 1]);
    let refused = crate::common::refusal(&over, 60.0, &crate::common::opts(0.1));
    assert_eq!(
        refused,
        SynthError::TooManyEqBands {
            found: MAX_EQ_BANDS + 1,
            limit: MAX_EQ_BANDS,
        }
    );
    assert!(
        refused
            .to_string()
            .contains(&format!("at most {MAX_EQ_BANDS} bands")),
        "the message names the cap: {refused}"
    );
}
