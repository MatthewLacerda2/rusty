//! The chorus end to end: what it is written down as, and the two things it
//! promises on real audio — that it widens what it is given, and that it does
//! so identically every time.

use crate::common::{channel, opts, render_stereo, saw_patch};
use scorsese_zimmer::patch::{Fx, Patch};
use scorsese_zimmer::{Patch as PatchDoc, bake_note};

/// A saw put through an ensemble. A saw rather than the noise blip other
/// document tests use: `noise` is already two uncorrelated signals, and a
/// fixture that arrives wide cannot say what the chorus did.
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

/// How alike the two channels of a rendered note are, `1.0` being identical.
fn correlation(patch: &Patch) -> f32 {
    let buf = render_stereo(patch, 57.0, &opts(1.0));
    let (left, right) = (channel(&buf, 0), channel(&buf, 1));
    let dot: f32 = left.iter().zip(&right).map(|(l, r)| l * r).sum();
    let energy = |c: &[f32]| c.iter().map(|s| s * s).sum::<f32>().sqrt();
    dot / (energy(&left) * energy(&right)).max(f32::MIN_POSITIVE)
}

/// The half of the effect that waited for the crate to go stereo: the copies
/// sit *outside* the dry signal, so a source that went in dead centre comes
/// back as something with two sides.
#[test]
fn a_source_that_went_in_centred_comes_back_wider_than_it_was() {
    let dry = correlation(&saw_patch());
    assert!((dry - 1.0).abs() < 1e-4, "the fixture is centred: {dry}");
    let wet = correlation(&chorused(4));
    assert!(wet < 0.9, "the sides are still {wet} alike");
    assert!(wet > -1.0, "and it is a spread, not an inversion");
}

/// Every legal ensemble is a wide one. The width is carried by the outermost
/// pair rather than by the count — a pair panned hard is the widest thing this
/// makes, and the voices between them fill the field in rather than stretching
/// it — so this is the claim at each size, not a claim that four beats two.
#[test]
fn every_size_of_ensemble_is_wide() {
    for voices in 2..=4 {
        let wet = correlation(&chorused(voices));
        assert!(wet < 0.9, "{voices} voices are still {wet} alike");
    }
}

/// A voice count is a thickness control and not a fader: the copies are
/// normalised across each side, so adding one does not turn the effect up and
/// hand the limiter a job it should not have.
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

/// The crate's contract, on the one effect that could most easily break it: a
/// free-running LFO, or a phase taken from a counter, would render a different
/// file on the second call. These are the *bytes*, not the samples.
#[test]
fn the_same_recipe_bakes_the_same_bytes_twice() {
    let patch = chorused(3);
    let once = bake_note(&patch, 57.0, &opts(0.5)).expect("bakes");
    let again = bake_note(&patch, 57.0, &opts(0.5)).expect("bakes");
    assert_eq!(once.wav, again.wav);
    // And across a round trip through the document, so nothing about the
    // ensemble is carried in the struct rather than in the recipe.
    let json = patch.to_json().expect("serialise");
    let reloaded = PatchDoc::from_json(&json).expect("deserialise");
    assert_eq!(
        bake_note(&reloaded, 57.0, &opts(0.5)).expect("bakes").wav,
        once.wav
    );
}

/// A chorus rings on by the deepest its line is read from and no further — a
/// fortieth of a second, against a reverb's seconds. A note is padded by it,
/// so getting this wrong grows every recipe that uses one.
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

/// `voices` is the one field with a default, because it is the one a recipe
/// has least reason to have an opinion about. Three is a section.
#[test]
fn an_omitted_voice_count_is_three() {
    let json = r#"{ "fx": "chorus", "rate": 0.7, "depth": 0.6, "mix": 0.5 }"#;
    let Fx::Chorus { voices, .. } = serde_json::from_str(json).expect("parses without voices")
    else {
        panic!("that is a chorus");
    };
    assert_eq!(voices, 3);
}

/// Out of range is clamped rather than refused, the way a pan past the edge is
/// the edge: there is no ensemble smaller than a pair or usefully larger than
/// a section for a bigger number to mean.
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
