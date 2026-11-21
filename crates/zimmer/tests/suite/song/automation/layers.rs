//! A `cutoff` curve on a layered instrument moves the top-level filter, and
//! only that one: a layer's own filter stays where the layer wrote it.

use scorsese_zimmer::patch::Layer;
use scorsese_zimmer::song::{InlineOnly, Param};
use scorsese_zimmer::{Patch, SynthError, render_song};

use super::setup::{at, curve, filtered, render, voiced};

/// A filtered saw over a layer of the same saw, filtered at `cutoff`.
fn layered(cutoff: f32) -> Patch {
    let below = filtered(cutoff);
    Patch {
        layers: vec![Layer {
            source: below.source,
            amp: below.amp,
            filter: below.filter,
            pitch_env: None,
            gain: 1.0,
        }],
        ..filtered(2000.0)
    }
}

#[test]
fn a_cutoff_curve_leaves_a_layer_s_filter_alone() {
    let swept = |cutoff| {
        let mut song = voiced(layered(cutoff), 0.5);
        song.automation = vec![curve(Param::Cutoff, vec![at(0.0, 300.0)])];
        render(&song)
    };
    // Had the curve moved the layer's filter too, both would sit at 300 Hz.
    assert_ne!(swept(300.0), swept(5000.0));
}

#[test]
fn an_instrument_filtered_only_in_its_layers_has_no_filter_to_move() {
    let patch = Patch {
        filter: None,
        ..layered(300.0)
    };
    let mut song = voiced(patch, 0.5);
    song.automation = vec![curve(Param::Cutoff, vec![at(0.0, 900.0)])];
    assert!(matches!(
        render_song(&song, &InlineOnly).expect_err("the top level has no filter"),
        SynthError::AutomationWithoutFilter { ref track } if track == "pad"
    ));
}
