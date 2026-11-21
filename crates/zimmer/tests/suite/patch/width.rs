//! Unison width: a widened oscillator's voices spread across the stereo field.

use crate::common::{channel, minimal, opts, osc, render_stereo};
use rusty_zimmer::patch::{Osc, Source, Wave};
use rusty_zimmer::Patch;

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

fn correlation(patch: &Patch) -> f32 {
    let both = render_stereo(patch, 57.0, &opts(0.5));
    let (l, r) = (channel(&both, 0), channel(&both, 1));
    let dot = |x: &[f32], y: &[f32]| x.iter().zip(y).map(|(p, q)| p * q).sum::<f32>();
    dot(&l, &r) / (dot(&l, &l) * dot(&r, &r)).sqrt()
}

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
