//! Every patch document type refuses a key it does not know, and says which.
//!
//! A misspelled key used to be dropped without a word and the field it meant
//! to set took its default: `"gian": 0.5` baked at unity gain, `"atack"` at
//! the default attack. No error, a plausible bake, a different sound (#592).
//! One test per type, each asserting the refusal **names** the key, because
//! a refusal that does not is only half the fix for an agent reading it.

use scorsese_zimmer::Patch;
use scorsese_zimmer::patch::{Adsr, EqBand, Fx, Lfo, Operator, Partial, PitchEnv, Source};
use serde::de::DeserializeOwned;

/// Asserts `json` does not parse as `T`, and that the refusal names `key`.
fn refused<T: DeserializeOwned + std::fmt::Debug>(json: &str, key: &str) {
    let refusal = serde_json::from_str::<T>(json)
        .expect_err("a misspelled key is refused")
        .to_string();
    assert!(
        refusal.contains(&format!("unknown field `{key}`")),
        "the refusal names `{key}`: {refusal}"
    );
}

#[test]
fn a_patch_refuses_an_unknown_stage() {
    refused::<Patch>(
        r#"{ "source": { "kind": "noise" }, "amp": { "a": 0, "d": 0, "s": 1, "r": 0 },
             "filtre": { "kind": "lowpass", "cutoff": 800 } }"#,
        "filtre",
    );
}

#[test]
fn an_oscillator_refuses_a_misspelled_gain() {
    refused::<Source>(
        r#"{ "kind": "osc_stack", "oscs": [ { "wave": "saw", "gian": 0.5 } ] }"#,
        "gian",
    );
}

/// The tag is `kind`, and it must not be reported as a stray key of the
/// variant it selects — so the well-spelled form parses first.
#[test]
fn a_source_refuses_a_key_its_kind_does_not_have() {
    let written = r#"{ "kind": "osc_stack", "oscs": [] }"#;
    serde_json::from_str::<Source>(written).expect("`kind` is the tag, not a stray key");
    refused::<Source>(r#"{ "kind": "osc_stack", "osc": [] }"#, "osc");
    refused::<Source>(r#"{ "kind": "karplus", "dampnig": 0.9 }"#, "dampnig");
}

#[test]
fn an_envelope_refuses_a_misspelled_attack() {
    refused::<Adsr>(
        r#"{ "atack": 0.1, "a": 0, "d": 0, "s": 1, "r": 0 }"#,
        "atack",
    );
}

#[test]
fn a_pitch_envelope_refuses_an_unknown_key() {
    refused::<PitchEnv>(r#"{ "semitones": -12, "adrs": {} }"#, "adrs");
}

#[test]
fn an_lfo_refuses_a_misspelled_rate() {
    refused::<Lfo>(r#"{ "rate": 5, "depht": 0.5, "target": "pitch" }"#, "depht");
}

#[test]
fn an_operator_refuses_a_misspelled_level() {
    refused::<Operator>(r#"{ "ratio": 1, "levle": 3 }"#, "levle");
}

#[test]
fn a_partial_refuses_a_misspelled_decay() {
    refused::<Partial>(r#"{ "ratio": 2, "decya": 0.4 }"#, "decya");
}

#[test]
fn an_eq_band_refuses_a_misspelled_frequency() {
    refused::<EqBand>(r#"{ "kind": "peak", "frequency": 250 }"#, "frequency");
}

/// The tag is `fx`, and the same care applies as for a source's `kind`.
#[test]
fn an_effect_refuses_a_key_its_kind_does_not_have() {
    let written = r#"{ "fx": "reverb", "size": 0.5, "damp": 0.5, "mix": 0.2 }"#;
    serde_json::from_str::<Fx>(written).expect("`fx` is the tag, not a stray key");
    refused::<Fx>(
        r#"{ "fx": "reverb", "size": 0.5, "damp": 0.5, "mix": 0.2, "dmap": 1 }"#,
        "dmap",
    );
}
