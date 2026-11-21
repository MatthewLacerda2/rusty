//! The chorus end to end: what it is written down as, and the two things it promises on real audio
//! — that it widens what it is given, and that it does so identically every time.

use crate::common::{channel, opts, render_stereo, saw_patch};
use scorsese_zimmer::patch::{Fx, Patch};
use scorsese_zimmer::{Patch as PatchDoc, bake_note};

fn chorused(voices: usize) -> Patch {
    Patch {
        fx: vec![Fx::Chorus {
            rate: 0.7,
            depth: 0.6,
            voices,
            mix: 1.0,
        }],
        ..saw_patch()
    }
}

fn correlation(patch: &Patch) -> f32 {
    let buf = render_stereo(patch, 57.0, &opts(1.0));
    let (left, right) = (channel(&buf, 0), channel(&buf, 1));
    let dot: f32 = left.iter().zip(&right).map(|(l, r)| l * r).sum();
    let energy = |c: &[f32]| c.iter().map(|s| s * s).sum::<f32>().sqrt();
    dot / (energy(&left) * energy(&right)).max(f32::MIN_POSITIVE)
}

#[test]
fn a_source_that_went_in_centred_comes_back_wider_than_it_was() {
    let dry = correlation(&saw_patch());
    assert!((dry - 1.0).abs() < 1e-4, "the fixture is centred: {dry}");
    let wet = correlation(&chorused(4));
    assert!(wet < 0.9, "the sides are still {wet} alike");
    assert!(wet > -1.0, "and it is a spread, not an inversion");
}

#[test]
fn every_size_of_ensemble_is_wide() {
    for voices in 2..=4 {
        let wet = correlation(&chorused(voices));
        assert!(wet < 0.9, "{voices} voices are still {wet} alike");
    }
}

#[test]
fn adding_a_voice_does_not_add_level() {
    let loudest = |voices| {
        let buf = render_stereo(&chorused(voices), 57.0, &opts(1.0));
        buf.iter().fold(0.0f32, |most, s| most.max(s.abs()))
    };
    let (pair, section) = (loudest(2), loudest(4));
    assert!(
        (section / pair - 1.0).abs() < 0.15,
        "two peak at {pair} and four at {section}"
    );
}

#[test]
fn the_same_recipe_bakes_the_same_bytes_twice() {
    let patch = chorused(3);
    let once = bake_note(&patch, 57.0, &opts(0.5)).expect("bakes");
    let again = bake_note(&patch, 57.0, &opts(0.5)).expect("bakes");
    assert_eq!(once.wav, again.wav);
    let json = patch.to_json().expect("serialise");
    let reloaded = PatchDoc::from_json(&json).expect("deserialise");
    assert_eq!(
        bake_note(&reloaded, 57.0, &opts(0.5)).expect("bakes").wav,
        once.wav
    );
}

#[test]
fn a_note_grows_by_a_fortieth_of_a_second_and_not_by_a_room() {
    let plain = render_stereo(&saw_patch(), 57.0, &opts(0.5)).len();
    let ensemble = render_stereo(&chorused(3), 57.0, &opts(0.5)).len();
    let grew = (ensemble - plain) as f32 / 2.0 / 44_100.0;
    assert!(
        (0.02..0.03).contains(&grew),
        "the note grew by {grew} seconds"
    );
}

#[test]
fn the_tag_and_the_fields_are_the_words_a_recipe_is_written_in() {
    let json = serde_json::to_string(&Fx::Chorus {
        rate: 0.7,
        depth: 0.6,
        voices: 3,
        mix: 0.5,
    })
    .expect("serialise");
    assert_eq!(
        json,
        r#"{"fx":"chorus","rate":0.7,"depth":0.6,"voices":3,"mix":0.5}"#
    );
}

#[test]
fn an_omitted_voice_count_is_three() {
    let json = r#"{ "fx": "chorus", "rate": 0.7, "depth": 0.6, "mix": 0.5 }"#;
    let Fx::Chorus { voices, .. } = serde_json::from_str(json).expect("parses without voices")
    else {
        panic!("that is a chorus");
    };
    assert_eq!(voices, 3);
}

#[test]
fn a_voice_count_past_either_end_is_that_end() {
    let bytes = |voices| {
        bake_note(&chorused(voices), 57.0, &opts(0.3))
            .expect("bakes")
            .wav
    };
    assert_eq!(bytes(0), bytes(2), "under a pair is a pair");
    assert_eq!(bytes(99), bytes(4), "over a section is a section");
}
