//! `levels` against values worked by hand.

use super::*;

fn levels(in_low: f32, in_high: f32, gamma: f32, out_low: f32, out_high: f32) -> Levels {
    Levels {
        in_low,
        in_high,
        gamma,
        out_low,
        out_high,
    }
}

fn close(got: f32, want: f32) {
    assert!((got - want).abs() < 1e-6, "{got} vs {want}");
}

#[test]
fn the_defaults_pass_through() {
    let l: Levels = serde_json::from_str("{}").unwrap();
    for c in [0.0, 0.3, 1.0] {
        close(l.level(c), c);
    }
}

#[test]
fn gamma_moves_the_midtones_and_keeps_the_ends() {
    let l = levels(0.0, 1.0, 2.0, 0.0, 1.0);
    close(l.level(0.25), 0.5);
    close(l.level(0.0), 0.0);
    close(l.level(1.0), 1.0);
}

#[test]
fn the_input_range_clamps_and_the_output_range_maps() {
    let l = levels(0.2, 0.6, 1.0, 0.1, 0.5);
    close(l.level(0.0), 0.1);
    close(l.level(0.4), 0.3);
    close(l.level(0.9), 0.5);
}

#[test]
fn an_empty_input_range_is_a_step() {
    let l = levels(0.5, 0.5, 1.0, 0.0, 1.0);
    close(l.level(0.49), 0.0);
    close(l.level(0.5), 1.0);
}
