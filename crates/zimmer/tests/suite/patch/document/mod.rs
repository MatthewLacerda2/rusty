//! Round-tripping and tag spelling; the defaults an omitted field means are in `defaults`.

use crate::common::{adsr, osc};
use rusty_zimmer::patch::{
    Adsr, Filter, FilterKind, Fx, Lfo, LfoTarget, PitchEnv, Slope, Source, Wave,
};
use rusty_zimmer::Patch;

mod defaults;

fn full_patch() -> Patch {
    Patch {
        layers: Vec::new(),
        source: Source::OscStack {
            oscs: vec![osc(Wave::Saw, -7.0, 0), osc(Wave::Square, 7.0, -1)],
        },
        amp: adsr(0.005, 0.1, 0.7, 0.3),
        filter: Some(Filter {
            kind: FilterKind::Lowpass,
            slope: Slope::Db12,
            cutoff: 1200.0,
            resonance: 0.4,
            env_octaves: 1.5,
            vel_octaves: 1.75,
            key_track: 0.0,
            adsr: adsr(0.01, 0.2, 0.3, 0.2),
        }),
        pitch_env: Some(PitchEnv {
            semitones: -12.0,
            adsr: Adsr {
                curve: 4.0,
                ..adsr(0.0, 0.04, 0.0, 0.0)
            },
        }),
        lfo: Some(Lfo::new(5.0, 0.5, LfoTarget::Pitch)),
        lfo2: None,
        fx: vec![
            Fx::Delay {
                time: 0.25,
                beats: None,
                feedback: 0.35,
                mix: 0.3,
                ping_pong: false,
            },
            Fx::Reverb {
                size: 0.6,
                damp: 0.5,
                mix: 0.2,
                predelay: 0.0,
            },
        ],
    }
}

#[test]
fn a_patch_round_trips_through_json_unchanged() {
    let patch = full_patch();
    let json = patch.to_json().expect("serialise");
    assert_eq!(Patch::from_json(&json).expect("deserialise"), patch);
}

#[test]
fn tags_are_snake_case_and_named_as_documented() {
    let json = serde_json::to_string(&Source::OscStack { oscs: vec![] }).expect("serialise");
    assert!(json.contains(r#""kind":"osc_stack""#), "got {json}");

    let json = serde_json::to_string(&Fx::Delay {
        time: 0.1,
        beats: None,
        feedback: 0.2,
        mix: 0.3,
        ping_pong: false,
    })
    .expect("serialise");
    assert!(json.contains(r#""fx":"delay""#), "got {json}");
}
