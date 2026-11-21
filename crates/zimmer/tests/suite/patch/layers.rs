//! Layers: several sounds summed into one note before the patch's fx chain —
//! how they are written down, what is refused, and what they render to.

use crate::common::{channel, peak, render_stereo};
use scorsese_zimmer::patch::MAX_LAYERS;
use scorsese_zimmer::{NoteOpts, Patch, SynthError};

/// A snare: a short pink-noise crack on top, a falling sine body beneath.
const SNARE: &str = r#"{
    "source": { "kind": "noise", "color": "pink" },
    "amp": { "a": 0.001, "d": 0.08, "s": 0.0, "r": 0.02 },
    "layers": [{
        "source": { "kind": "osc_stack", "oscs": [{ "wave": "sine" }] },
        "amp": { "a": 0.001, "d": 0.12, "s": 0.0, "r": 0.3 },
        "pitch_env": { "semitones": 7, "adsr": { "a": 0.0, "d": 0.03, "s": 0.0, "r": 0.0 } },
        "gain": 0.8
    }]
}"#;

fn snare() -> Patch {
    Patch::from_json(SNARE).expect("the snare parses")
}

fn hit() -> NoteOpts {
    NoteOpts {
        duration: 0.05,
        velocity: 1.0,
        timbre: 0.0,
        glide: None,
        seed: 3,
    }
}

fn refused(patch: &Patch) -> SynthError {
    patch.validate().expect_err("the patch is refused")
}

#[test]
fn a_layered_patch_round_trips_without_writing_a_default() {
    let patch = snare();
    let back = Patch::from_json(&patch.to_json().expect("writes")).expect("reads back");
    assert_eq!(back, patch);
    let quiet = SNARE.replace(r#""gain": 0.8"#, r#""gain": 1.0"#);
    let written = Patch::from_json(&quiet).unwrap().to_json().unwrap();
    let layer = &serde_json::from_str::<serde_json::Value>(&written).unwrap()["layers"][0];
    assert!(
        layer.get("gain").is_none(),
        "unity gain was written: {layer}"
    );
    let unwritten = SNARE.replace(",\n        \"gain\": 0.8", "");
    assert_eq!(Patch::from_json(&unwritten).unwrap().layers[0].gain, 1.0);
    let bare = Patch {
        layers: Vec::new(),
        ..patch
    };
    assert!(!bare.to_json().unwrap().contains("layers"));
}

#[test]
fn a_layer_is_not_a_patch_and_holds_no_layers_or_chain() {
    for extra in [r#""layers": []"#, r#""fx": []"#, r#""lfo": null"#] {
        let written = SNARE.replace(r#""gain": 0.8"#, &format!(r#""gain": 0.8, {extra}"#));
        assert!(Patch::from_json(&written).is_err(), "took {extra}");
    }
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

#[test]
fn the_note_rings_as_long_as_its_longest_layer() {
    let layered = render_stereo(&snare(), 60.0, &hit()).len() / 2;
    let top = render_stereo(
        &Patch {
            layers: Vec::new(),
            ..snare()
        },
        60.0,
        &hit(),
    )
    .len()
        / 2;
    // 0.05 s held, then the body's 0.3 s release rather than the crack's 0.02.
    assert_eq!(layered, ((0.05f32 + 0.3) * 44_100.0).ceil() as usize);
    assert!(layered > top * 4, "{layered} against {top}");
}

#[test]
fn two_noise_layers_are_not_the_same_noise() {
    let mut patch = snare();
    patch.layers[0].source = patch.source.clone();
    patch.layers[0].amp = patch.amp;
    patch.layers[0].pitch_env = None;
    patch.layers[0].gain = 1.0;
    let both = channel(&render_stereo(&patch, 60.0, &hit()), 0);
    let one = channel(
        &render_stereo(
            &Patch {
                layers: Vec::new(),
                ..patch
            },
            60.0,
            &hit(),
        ),
        0,
    );
    // The same noise twice would be exactly double; two draws add in power.
    let ratio = peak(&both) / peak(&one);
    assert!(ratio < 1.9, "the layer replayed the top's noise: {ratio}");
}
