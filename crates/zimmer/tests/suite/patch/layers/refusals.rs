//! What a layered patch refuses: past the layer cap, a silent layer, and a layer's own fault by
//! its index.

use rusty_zimmer::patch::MAX_LAYERS;
use rusty_zimmer::{Patch, SynthError};

use super::{snare, SNARE};

fn refused(patch: &Patch) -> SynthError {
    patch.validate().expect_err("the patch is refused")
}

#[test]
fn past_the_cap_is_refused() {
    let mut patch = snare();
    patch.layers = vec![patch.layers[0].clone(); MAX_LAYERS + 1];
    assert_eq!(
        refused(&patch),
        SynthError::TooManyLayers {
            found: MAX_LAYERS + 1,
            limit: MAX_LAYERS
        }
    );
    patch.layers.pop();
    patch.validate().expect("the cap itself is allowed");
}

#[test]
fn a_silent_layer_is_refused_by_its_index() {
    let mut patch = snare();
    patch.layers.push(patch.layers[0].clone());
    patch.layers[1].gain = 0.0;
    let error = refused(&patch);
    assert_eq!(
        error,
        SynthError::InLayer {
            layer: 1,
            error: Box::new(SynthError::SilentLayer)
        }
    );
    assert!(error.to_string().contains("layers[1]"), "{error}");
}

#[test]
fn a_layer_s_own_refusal_names_the_layer() {
    let empty = SNARE.replace(r#"[{ "wave": "sine" }]"#, "[]");
    let error = refused(&Patch::from_json(&empty).unwrap());
    assert_eq!(
        error,
        SynthError::InLayer {
            layer: 0,
            error: Box::new(SynthError::EmptyOscStack)
        }
    );
    let words = error.to_string();
    assert!(
        words.contains("layers[0]") && words.contains("osc_stack"),
        "{words}"
    );
}
