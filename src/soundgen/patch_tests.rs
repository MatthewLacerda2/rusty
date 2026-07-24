//! Tests for the patch document: round-tripping, tag spelling, defaults, and the
//! validation that stops an unplayable patch before any samples are rendered.

use super::*;

/// One oscillator, spelled out — the stack entries the round-trip patch carries.
fn osc(wave: Wave, detune_cents: f32, octave: i32) -> Osc {
    Osc {
        wave,
        detune_cents,
        gain: 0.5,
        octave,
    }
}

fn adsr(a: f32, d: f32, s: f32, r: f32) -> Adsr {
    Adsr { a, d, s, r }
}

/// A representative patch touching every optional stage, used by the round-trip.
fn full_patch() -> Patch {
    Patch {
        source: Source::OscStack {
            oscs: vec![osc(Wave::Saw, -7.0, 0), osc(Wave::Square, 7.0, -1)],
        },
        amp: adsr(0.005, 0.1, 0.7, 0.3),
        filter: Some(Filter {
            kind: FilterKind::Lowpass,
            cutoff: 1200.0,
            resonance: 0.4,
            env_amount: 2000.0,
            adsr: adsr(0.01, 0.2, 0.3, 0.2),
        }),
        lfo: Some(Lfo {
            rate: 5.0,
            depth: 0.5,
            target: LfoTarget::Pitch,
        }),
        fx: vec![
            Fx::Delay {
                time: 0.25,
                feedback: 0.35,
                mix: 0.3,
            },
            Fx::Reverb {
                size: 0.6,
                damp: 0.5,
                mix: 0.2,
            },
        ],
    }
}

#[test]
fn patch_round_trips_through_json_unchanged() {
    let p = full_patch();
    let json = p.to_json().expect("serialize");
    assert_eq!(Patch::from_json(&json).expect("deserialize"), p);
}

#[test]
fn tags_are_snake_case_and_named_as_documented() {
    let json = serde_json::to_string(&Source::OscStack { oscs: vec![] }).unwrap();
    assert!(json.contains(r#""kind":"osc_stack""#), "got {json}");
    let json = serde_json::to_string(&Fx::Delay {
        time: 0.1,
        feedback: 0.2,
        mix: 0.3,
    })
    .unwrap();
    assert!(json.contains(r#""fx":"delay""#), "got {json}");
}

#[test]
fn optional_stages_and_osc_fields_have_defaults() {
    let json = r#"{
        "source": { "kind": "osc_stack", "oscs": [ { "wave": "saw" } ] },
        "amp": { "a": 0.01, "d": 0.1, "s": 0.5, "r": 0.2 }
    }"#;
    let p = Patch::from_json(json).expect("parses without the optional stages");
    assert!(p.filter.is_none() && p.lfo.is_none() && p.fx.is_empty());
    match &p.source {
        Source::OscStack { oscs } => {
            assert_eq!(oscs[0].gain, 1.0, "gain defaults to unity");
            assert_eq!(oscs[0].detune_cents, 0.0);
            assert_eq!(oscs[0].octave, 0);
        }
        other => panic!("wrong source: {other:?}"),
    }
}

#[test]
fn karplus_and_fm_carry_their_documented_defaults() {
    let json = r#"{ "kind": "karplus" }"#;
    match serde_json::from_str::<Source>(json).unwrap() {
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
    match serde_json::from_str::<Source>(json).unwrap() {
        Source::Fm2 { mod_decay, .. } => assert_eq!(mod_decay, 0.3),
        other => panic!("wrong source: {other:?}"),
    }
}

/// The smallest legal patch, used as the base for the validation cases.
fn minimal(source: Source) -> Patch {
    Patch {
        source,
        amp: Adsr::default(),
        filter: None,
        lfo: None,
        fx: vec![],
    }
}

#[test]
fn validation_accepts_every_source_kind() {
    for source in [
        Source::OscStack {
            oscs: vec![Osc {
                wave: Wave::Sine,
                detune_cents: 0.0,
                gain: 1.0,
                octave: 0,
            }],
        },
        Source::Karplus {
            damping: 0.99,
            brightness: 0.5,
        },
        Source::Noise,
        Source::Fm2 {
            ratio: 2.0,
            index: 3.0,
            mod_decay: 0.3,
        },
    ] {
        minimal(source).validate().expect("legal patch");
    }
}

#[test]
fn validation_rejects_a_stack_that_cannot_make_sound() {
    let silent = Osc {
        wave: Wave::Saw,
        detune_cents: 0.0,
        gain: 0.0,
        octave: 0,
    };
    assert!(minimal(Source::OscStack { oscs: vec![] })
        .validate()
        .is_err());
    assert!(minimal(Source::OscStack {
        oscs: vec![silent; MAX_OSCS + 1]
    })
    .validate()
    .is_err());
    assert!(minimal(Source::OscStack { oscs: vec![silent] })
        .validate()
        .is_err());
}

#[test]
fn validation_rejects_unrenderable_stage_settings() {
    let mut p = minimal(Source::Noise);
    p.filter = Some(Filter {
        kind: FilterKind::Lowpass,
        cutoff: 0.0,
        resonance: 0.0,
        env_amount: 0.0,
        adsr: Adsr::default(),
    });
    assert!(p.validate().is_err(), "a zero cutoff is not a filter");

    let mut p = minimal(Source::Noise);
    p.lfo = Some(Lfo {
        rate: -1.0,
        depth: 0.5,
        target: LfoTarget::Amp,
    });
    assert!(p.validate().is_err(), "negative rate is not a rate");

    assert!(minimal(Source::Fm2 {
        ratio: 0.0,
        index: 1.0,
        mod_decay: 0.3,
    })
    .validate()
    .is_err());
}
