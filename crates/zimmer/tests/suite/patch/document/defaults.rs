//! The defaults an omitted field means, and the defaults left unwritten on the way back out.

use rusty_zimmer::patch::{Filter, Source};
use rusty_zimmer::Patch;

#[test]
fn optional_stages_and_osc_fields_have_defaults() {
    let json = r#"{
        "source": { "kind": "osc_stack", "oscs": [ { "wave": "saw" } ] },
        "amp": { "a": 0.01, "d": 0.1, "s": 0.5, "r": 0.2 }
    }"#;
    let patch = Patch::from_json(json).expect("parses without the optional stages");
    assert!(patch.filter.is_none() && patch.lfo.is_none() && patch.fx.is_empty());
    match &patch.source {
        Source::OscStack { oscs } => {
            assert_eq!(oscs[0].gain, 1.0, "gain defaults to unity");
            assert_eq!(oscs[0].detune_cents, 0.0);
            assert_eq!(oscs[0].octave, 0);
        }
        other => panic!("wrong source: {other:?}"),
    }
}

#[test]
fn velocity_routing_is_off_unless_a_recipe_asks_for_it() {
    let json = r#"{ "kind": "lowpass", "cutoff": 800 }"#;
    let filter: Filter = serde_json::from_str(json).expect("parses");
    assert_eq!(
        filter.vel_octaves, 0.0,
        "velocity opens the cutoff by nothing"
    );

    let json = r#"{ "kind": "fm2", "ratio": 2.0, "index": 3.0 }"#;
    match serde_json::from_str::<Source>(json).expect("parses") {
        Source::Fm2 { vel_index, .. } => assert_eq!(vel_index, 0.0, "nor any FM depth"),
        other => panic!("wrong source: {other:?}"),
    }
}

#[test]
fn a_straight_envelope_and_an_unswept_pitch_are_absent_from_the_document() {
    let json = r#"{
        "source": { "kind": "noise" },
        "amp": { "a": 0.01, "d": 0.1, "s": 0.5, "r": 0.2 }
    }"#;
    let patch = Patch::from_json(json).expect("parses without either");
    assert_eq!(patch.amp.curve, 0.0, "an envelope is straight by default");
    assert!(patch.pitch_env.is_none(), "and the pitch holds still");

    let saved = patch.to_json().expect("serialise");
    assert!(!saved.contains("curve"), "curve written back: {saved}");
    assert!(
        !saved.contains("pitch_env"),
        "pitch_env written back: {saved}"
    );

    let bent = Patch::from_json(&saved.replace(r#""s": 0.5"#, r#""s": 0.5, "curve": 3.0"#))
        .expect("parses with a curve");
    assert_eq!(bent.amp.curve, 3.0, "and a curve that is asked for arrives");
    assert!(
        bent.to_json().expect("serialise").contains("curve"),
        "and survives the round trip"
    );
}

#[test]
fn karplus_and_fm_carry_their_documented_defaults() {
    let json = r#"{ "kind": "karplus" }"#;
    match serde_json::from_str::<Source>(json).expect("parses") {
        Source::Karplus {
            damping,
            brightness,
        } => {
            assert_eq!(damping, 0.996);
            assert_eq!(brightness, 0.5);
        }
        other => panic!("wrong source: {other:?}"),
    }

    let json = r#"{ "kind": "fm2", "ratio": 2.0, "index": 3.0 }"#;
    match serde_json::from_str::<Source>(json).expect("parses") {
        Source::Fm2 { mod_decay, .. } => assert_eq!(mod_decay, 0.3),
        other => panic!("wrong source: {other:?}"),
    }
}
