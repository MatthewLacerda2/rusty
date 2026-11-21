//! What a velocity does besides move the fader.

use crate::common::{brightness, minimal, opts, peak, render, saw_patch};
use rusty_zimmer::patch::{Adsr, Filter, FilterKind, Patch, Slope, Source};
use rusty_zimmer::NoteOpts;

mod timbre;

fn struck(patch: &Patch, velocity: f32) -> Vec<f32> {
    render(
        patch,
        45.0,
        &NoteOpts {
            velocity,
            ..opts(0.5)
        },
    )
}

fn lowpass(cutoff: f32, vel_octaves: f32) -> Filter {
    Filter {
        kind: FilterKind::Lowpass,
        slope: Slope::Db12,
        cutoff,
        resonance: 0.0,
        env_octaves: 0.0,
        vel_octaves,
        key_track: 0.0,
        adsr: Adsr::default(),
    }
}

fn fm(index: f32, vel_index: f32) -> Patch {
    minimal(Source::Fm2 {
        ratio: 2.0,
        index,
        vel_index,
        mod_decay: 10.0,
    })
}

#[test]
fn vel_octaves_makes_a_hard_strike_brighter_than_a_soft_one() {
    let mut patch = saw_patch();
    patch.filter = Some(lowpass(400.0, 3.5));
    let soft = brightness(&struck(&patch, 0.2));
    let hard = brightness(&struck(&patch, 1.0));
    assert!(
        hard > soft * 2.0,
        "a hard strike opens the filter: {soft} → {hard}"
    );
}

#[test]
fn vel_index_makes_a_hard_strike_brighter_on_fm() {
    let patch = fm(1.0, 8.0);
    let soft = brightness(&struck(&patch, 0.1));
    let hard = brightness(&struck(&patch, 1.0));
    assert!(
        hard > soft * 2.0,
        "a hard strike adds sidebands: {soft} → {hard}"
    );
}

#[test]
fn velocity_alone_is_still_only_a_fader() {
    let mut patch = saw_patch();
    patch.filter = Some(lowpass(1200.0, 0.0));
    let hard = struck(&patch, 1.0);
    let soft = struck(&patch, 0.25);
    assert!(peak(&hard) > 0.1, "there is a signal to compare at all");
    for (loud, quiet) in hard.iter().zip(&soft) {
        assert!(
            (loud * 0.25 - quiet).abs() < 1e-6,
            "the soft note is the loud one scaled, not a different sound"
        );
    }
}

#[test]
fn a_negative_vel_octaves_darkens_instead_of_brightening() {
    let mut patch = saw_patch();
    patch.filter = Some(lowpass(6000.0, -5.0));
    let soft = brightness(&struck(&patch, 0.1));
    let hard = brightness(&struck(&patch, 1.0));
    assert!(
        hard < soft * 0.5,
        "harder is darker when the routing says so: {soft} → {hard}"
    );
}

#[test]
fn an_extreme_vel_octaves_cannot_destabilise_the_filter() {
    for vel_octaves in [-1e9, 1e9] {
        let mut patch = saw_patch();
        patch.filter = Some(Filter {
            resonance: 0.99,
            ..lowpass(1000.0, vel_octaves)
        });
        let buf = struck(&patch, 1.0);
        assert!(
            buf.iter().all(|sample| sample.is_finite()),
            "vel_octaves {vel_octaves} produced non-finite samples"
        );
    }
}

#[test]
fn a_negative_vel_index_bottoms_out_rather_than_brightening_again() {
    let dulled = brightness(&struck(&fm(2.0, -1.0), 1.0));
    let far_past = brightness(&struck(&fm(2.0, -8.0), 1.0));
    assert!(
        far_past < dulled,
        "past zero it is a plain carrier, not a bright one: {dulled} → {far_past}"
    );
}
