//! Unison width: a widened oscillator's voices spread across the stereo field
//! (#1124).
//!
//! What the document says and what the rendered note does with it — as against
//! `core::osc::unison`, where each voice's place on the pan axis is pinned.

use crate::common::{channel, minimal, opts, osc, render_stereo};
use scorsese_zimmer::Patch;
use scorsese_zimmer::patch::{Osc, Source, Wave};

/// A stack of one five-voice saw, fanned out `width` across the field.
fn stack(width: f32) -> Patch {
    minimal(Source::OscStack {
        oscs: vec![Osc {
            voices: 5,
            spread: 25.0,
            width,
            ..osc(Wave::Saw, 0.0, 0)
        }],
    })
}

/// Pearson correlation of a note's two sides: `1` is mono, `0` is two
/// unrelated signals.
fn correlation(patch: &Patch) -> f32 {
    let both = render_stereo(patch, 57.0, &opts(0.5));
    let (l, r) = (channel(&both, 0), channel(&both, 1));
    let dot = |x: &[f32], y: &[f32]| x.iter().zip(y).map(|(p, q)| p * q).sum::<f32>();
    dot(&l, &r) / (dot(&l, &l) * dot(&r, &r)).sqrt()
}

/// The default is no width, and no width is written into a document — so a
/// recipe from before the field keeps its bytes, its cached bake and its
/// identical channels.
#[test]
fn an_unspread_unison_is_written_without_width_and_stays_centred() {
    let json = stack(0.0).to_json().expect("serialise");
    assert!(!json.contains("width"), "the default width is in {json}");
    let both = render_stereo(&stack(0.0), 57.0, &opts(0.5));
    assert_eq!(channel(&both, 0), channel(&both, 1));
}

#[test]
fn a_wide_unison_round_trips_as_written() {
    let patch = stack(0.8);
    let json = patch.to_json().expect("serialise");
    assert!(json.contains("\"width\": 0.8"), "{json}");
    assert_eq!(Patch::from_json(&json).expect("deserialise"), patch);
}

/// The field's whole job, through the whole signal path: the two sides part
/// company, and the wider the stack the further apart they are.
#[test]
fn widening_a_unison_decorrelates_its_two_sides() {
    let half = correlation(&stack(0.5));
    let full = correlation(&stack(1.0));
    assert!(half < 0.99, "half width left the sides at {half}");
    assert!(
        full < half,
        "full width ({full}) is no wider than half ({half})"
    );
    assert!(full < 0.6, "full width only reached {full}");
}
