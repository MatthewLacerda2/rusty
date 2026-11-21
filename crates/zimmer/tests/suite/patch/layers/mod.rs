//! Layers: several sounds summed into one note before the patch's fx chain — how they are written
//! down and what they render to; what is refused is in `refusals`.

use crate::common::{channel, peak, render_stereo};
use rusty_zimmer::{NoteOpts, Patch};

mod refusals;

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
    let ratio = peak(&both) / peak(&one);
    assert!(ratio < 1.9, "the layer replayed the top's noise: {ratio}");
}
