//! Blend-tree weights (#457): 1D at thresholds and midpoints, 2D weights summing
//! to 1 and peaking at their own child, and the nearest-child fallback.

use std::collections::BTreeMap;

use glam::Vec2;

use super::*;
use crate::asset::animation_graph::{BlendChild1D, BlendChild2D};

fn approx(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

fn sum(w: &[f32]) -> f32 {
    w.iter().sum()
}

#[test]
fn one_d_plays_the_child_at_its_threshold_and_splits_midpoints() {
    // idle 0, walk 2, run 6 — authored out of order: the weights still follow
    // the thresholds, not the list.
    let t = [2.0, 0.0, 6.0];
    assert_eq!(weights_1d(&t, 0.0), vec![0.0, 1.0, 0.0]);
    assert_eq!(weights_1d(&t, 2.0), vec![1.0, 0.0, 0.0]);
    assert_eq!(weights_1d(&t, 6.0), vec![0.0, 0.0, 1.0]);
    assert_eq!(
        weights_1d(&t, 4.0),
        vec![0.5, 0.0, 0.5],
        "4 is half walk, half run"
    );
    assert_eq!(weights_1d(&t, 1.0), vec![0.5, 0.5, 0.0]);
    // Past either end the end child plays alone.
    assert_eq!(weights_1d(&t, -3.0), vec![0.0, 1.0, 0.0]);
    assert_eq!(weights_1d(&t, 9.0), vec![0.0, 0.0, 1.0]);
}

/// Idle at the origin plus the eight directions of a walk ring.
fn ring() -> Vec<Vec2> {
    let mut points = vec![Vec2::ZERO];
    for i in 0..8 {
        let a = i as f32 * std::f32::consts::FRAC_PI_4;
        points.push(Vec2::new(a.sin(), a.cos()));
    }
    points
}

#[test]
fn two_d_weights_sum_to_one_and_peak_at_their_own_child() {
    for polar in [true, false] {
        let points = ring();
        for (k, &p) in points.iter().enumerate() {
            let w = gradient_band(&points, p, polar);
            assert!(approx(w[k], 1.0), "child {k} plays alone at its point");
            assert!(
                approx(sum(&w), 1.0),
                "only child {k} weighs (polar {polar})"
            );
        }
        // Anywhere in between, the normalized weights sum to 1.
        let tree = BlendTree::FreeformDirectional2D {
            parameter_x: "x".into(),
            parameter_y: "y".into(),
            children: points
                .iter()
                .enumerate()
                .map(|(i, p)| BlendChild2D {
                    clip: format!("c{i}"),
                    position: p.to_array(),
                })
                .collect(),
        };
        for (x, y) in [(0.3, 0.4), (-0.7, 0.1), (0.5, -0.5), (0.0, 0.0), (3.0, 3.0)] {
            let params = BTreeMap::from([
                ("x".to_string(), AnimatorParameter::Float(x)),
                ("y".to_string(), AnimatorParameter::Float(y)),
            ]);
            let w = tree_weights(&tree, &params);
            assert!(
                approx(sum(&w), 1.0),
                "weights at ({x}, {y}) sum to {}",
                sum(&w)
            );
            assert!(w.iter().all(|&v| v >= 0.0));
        }
    }
}

#[test]
fn directional_between_forward_and_strafe_blends_only_those_two() {
    let points = vec![Vec2::Y, Vec2::X, -Vec2::Y, -Vec2::X];
    let diagonal = Vec2::new(1.0, 1.0).normalize();
    let w = gradient_band(&points, diagonal, true);
    let total = sum(&w);
    let (fwd, right) = (w[0] / total, w[1] / total);
    assert!(approx(fwd, 0.5) && approx(right, 0.5), "got {w:?}");
    assert!(
        approx(w[2], 0.0) && approx(w[3], 0.0),
        "back and left stay out"
    );
}

#[test]
fn directional_idle_fades_out_with_speed() {
    let points = vec![Vec2::ZERO, Vec2::new(0.0, 2.0)];
    let w = gradient_band(&points, Vec2::new(0.0, 0.5), true);
    assert!(
        approx(w[0], 0.75) && approx(w[1], 0.25),
        "a quarter of the way: {w:?}"
    );
}

#[test]
fn unset_parameters_read_zero_and_a_far_point_falls_back_to_the_nearest() {
    let tree = BlendTree::Simple1D {
        parameter: "speed".into(),
        children: vec![
            BlendChild1D {
                clip: "Walk".into(),
                threshold: 2.0,
            },
            BlendChild1D {
                clip: "Idle".into(),
                threshold: 0.0,
            },
        ],
    };
    assert_eq!(tree_weights(&tree, &BTreeMap::new()), vec![0.0, 1.0]);
    let cartesian = BlendTree::FreeformCartesian2D {
        parameter_x: "x".into(),
        parameter_y: "y".into(),
        children: vec![
            BlendChild2D {
                clip: "A".into(),
                position: [0.0, 0.0],
            },
            BlendChild2D {
                clip: "B".into(),
                position: [1.0, 0.0],
            },
        ],
    };
    let far = BTreeMap::from([("x".to_string(), AnimatorParameter::Float(5.0))]);
    assert_eq!(tree_weights(&cartesian, &far), vec![0.0, 1.0]);
}
