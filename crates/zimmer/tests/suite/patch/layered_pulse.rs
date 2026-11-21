//! Pulse width inside a layer (#1771 over #1773): a layer's square is moved
//! by the patch's pulse-width LFO like the top level's, and a width written
//! on a layer's other wave is refused by the layer's index.

use crate::common::render;
use scorsese_zimmer::patch::{LfoTarget, Wave};
use scorsese_zimmer::{NoteOpts, Patch, SynthError};

/// Noise on top, a 25% square in the one layer, and a pulse-width LFO.
const BUZZ: &str = r#"{
    "source": { "kind": "noise" },
    "amp": { "a": 0.001, "d": 0.1, "s": 0.5, "r": 0.05 },
    "layers": [{
        "source": { "kind": "osc_stack", "oscs": [{ "wave": "square", "pulse_width": 0.25 }] },
        "amp": { "a": 0.001, "d": 0.1, "s": 0.8, "r": 0.05 }
    }],
    "lfo": { "rate": 3.0, "depth": 0.2, "target": "pulse_width" }
}"#;

fn buzz() -> Patch {
    Patch::from_json(BUZZ).expect("parses")
}

fn held() -> NoteOpts {
    NoteOpts {
        duration: 0.4,
        velocity: 1.0,
        timbre: 0.0,
        glide: None,
        seed: 9,
    }
}

#[test]
fn a_square_only_in_a_layer_gives_the_lfo_something_to_move() {
    let patch = buzz();
    patch.validate().expect("a layer's square counts");
    let mut still = patch.clone();
    still.lfo = None;
    assert_ne!(render(&patch, 57.0, &held()), render(&still, 57.0, &held()));
}

#[test]
fn with_no_square_anywhere_the_lfo_is_refused() {
    let mut patch = buzz();
    let mut no_square = patch.layers[0].clone();
    no_square.source = Patch::from_json(
        r#"{ "source": { "kind": "noise" }, "amp": { "a": 0, "d": 0, "s": 1, "r": 0 } }"#,
    )
    .expect("parses")
    .source;
    patch.layers[0] = no_square;
    assert_eq!(patch.lfo.map(|l| l.target), Some(LfoTarget::PulseWidth));
    assert_eq!(patch.validate(), Err(SynthError::NoPulseToModulate));
}

#[test]
fn a_width_on_a_layer_s_saw_is_refused_by_its_index() {
    let json = BUZZ.replace(r#""wave": "square""#, r#""wave": "saw""#);
    let error = Patch::from_json(&json).unwrap().validate().unwrap_err();
    assert_eq!(
        error,
        SynthError::InLayer {
            layer: 0,
            error: Box::new(SynthError::PulseWidthOffSquare { wave: Wave::Saw })
        }
    );
}
