//! The amp envelope's bend: a curve changes how a note dies, never how hard it strikes.

use crate::common::{adsr, opts, peak, render, saw_patch};
use rusty_zimmer::patch::{Adsr, Patch};

use super::WINDOW;

const STRIKE: usize = 338;

fn decaying(curve: f32) -> Patch {
    Patch {
        amp: Adsr {
            curve,
            ..adsr(0.0, 1.0, 0.0, 0.0)
        },
        ..saw_patch()
    }
}

#[test]
fn a_curved_decay_is_further_down_than_a_straight_one() {
    let line = render(&decaying(0.0), 60.0, &opts(1.0));
    let bent = render(&decaying(4.0), 60.0, &opts(1.0));

    let (line_mid, bent_mid) = (halfway(&line), halfway(&bent));
    assert!(
        bent_mid < line_mid * 0.5,
        "halfway down: {line_mid} vs {bent_mid}"
    );

    let (line_hit, bent_hit) = (peak(&line[..STRIKE]), peak(&bent[..STRIKE]));
    assert!(
        (bent_hit - line_hit).abs() < line_hit * 0.03,
        "but the strike itself is as loud as it was: {line_hit} vs {bent_hit}"
    );
}

#[test]
fn the_opening_cycles_already_hold_the_loudest_sample_of_the_note() {
    for curve in [0.0, 4.0] {
        let buf = render(&decaying(curve), 60.0, &opts(1.0));
        assert_eq!(peak(&buf[..STRIKE]), peak(&buf), "curve {curve}");
    }
}

#[test]
fn a_negative_curve_holds_its_level_instead_of_shedding_it() {
    let line = render(&decaying(0.0), 60.0, &opts(1.0));
    let held = render(&decaying(-4.0), 60.0, &opts(1.0));
    assert!(
        halfway(&held) > halfway(&line) * 1.5,
        "halfway down: {} vs {}",
        halfway(&held),
        halfway(&line)
    );
}

fn halfway(buf: &[f32]) -> f32 {
    peak(&buf[22_050..22_050 + WINDOW])
}

#[test]
fn no_curve_renders_silence_or_a_non_finite_sample() {
    for curve in [-8.0, -3.0, 0.0, 3.0, 8.0] {
        let buf = render(&decaying(curve), 60.0, &opts(1.0));
        assert!(peak(&buf[..WINDOW]) > 0.5, "curve {curve} rendered nothing");
        assert!(
            buf.iter().all(|sample| sample.is_finite()),
            "curve {curve} went non-finite"
        );
    }
}
