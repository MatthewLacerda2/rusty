//! `histogram_scan` on a ramp of values: its ends, its pass-through and its step.

use super::*;

const RAMP: [f32; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];

fn scanned(position: f32, contrast: f32) -> Vec<f32> {
    RAMP.iter().map(|&c| scan(c, position, contrast)).collect()
}

#[test]
fn position_runs_from_all_black_to_all_white_at_any_contrast() {
    for contrast in [0.0, 0.5, 1.0] {
        assert_eq!(scanned(0.0, contrast), [0.0; 5], "contrast {contrast}");
        assert_eq!(scanned(1.0, contrast), [1.0; 5], "contrast {contrast}");
    }
}

#[test]
fn the_middle_with_no_contrast_passes_the_input_through() {
    assert_eq!(scanned(0.5, 0.0), RAMP);
}

#[test]
fn full_contrast_is_a_hard_threshold_at_one_minus_position() {
    assert_eq!(scanned(0.4, 1.0), [0.0, 0.0, 0.0, 1.0, 1.0]);
    assert_eq!(scanned(0.6, 1.0), [0.0, 0.0, 1.0, 1.0, 1.0]);
}

#[test]
fn coverage_grows_with_position() {
    let white = |p| scanned(p, 0.8).iter().sum::<f32>();
    assert!(white(0.3) < white(0.5) && white(0.5) < white(0.7));
}
