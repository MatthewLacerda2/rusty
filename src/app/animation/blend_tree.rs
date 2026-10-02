//! src/app/animation/blend_tree.rs — blend-tree weights (#457).
//!
//! How much each child of a [`BlendTree`] plays, from the `Float` parameters it
//! reads. The weights always sum to 1 (or are all 0 for a childless tree):
//! - **1D:** the two children whose thresholds bracket the value cross-blend
//!   linearly; past either end, the end child plays alone.
//! - **2D freeform:** gradient-band interpolation (Johansen, *Automated
//!   Semi-Procedural Animation*, 2009), which is what Unity's freeform trees use.
//!   Each child's influence falls off linearly toward every other child, and the
//!   smallest of those is its weight before normalizing. The *directional* kind
//!   measures that in polar space (speed difference, angle), so blending forward
//!   with a strafe turns the direction instead of shrinking the speed; the
//!   *cartesian* kind uses plain x/y.
//!
//! Pure functions of (tree, parameter values) — no state, deterministic.

use glam::Vec2;

use crate::asset::animation_graph::BlendTree;
use crate::components::{AnimatorParameter, AnimatorParameters};

/// How strongly the angle counts against the speed difference in the polar
/// space. Johansen's value, and the one Unity's directional trees behave like.
const ANGLE_WEIGHT: f32 = 2.0;

/// Below this a length is zero (a child at the origin, two coincident points).
const EPSILON: f32 = 1e-6;

/// Each child's weight, in authored order. A parameter that is unset or not a
/// `Float` reads 0.
pub fn tree_weights(tree: &BlendTree, parameters: &AnimatorParameters) -> Vec<f32> {
    let read = |name: &str| match parameters.get(name) {
        Some(AnimatorParameter::Float(v)) if v.is_finite() => *v,
        _ => 0.0,
    };
    let (points, at, weights) = match tree {
        BlendTree::Simple1D {
            parameter,
            children,
        } => {
            let thresholds: Vec<f32> = children.iter().map(|c| c.threshold).collect();
            let x = read(parameter);
            let points = thresholds.iter().map(|&t| Vec2::new(t, 0.0)).collect();
            (points, Vec2::new(x, 0.0), weights_1d(&thresholds, x))
        }
        BlendTree::FreeformDirectional2D {
            parameter_x,
            parameter_y,
            children,
        }
        | BlendTree::FreeformCartesian2D {
            parameter_x,
            parameter_y,
            children,
        } => {
            let points: Vec<Vec2> = children.iter().map(|c| Vec2::from(c.position)).collect();
            let at = Vec2::new(read(parameter_x), read(parameter_y));
            let polar = matches!(tree, BlendTree::FreeformDirectional2D { .. });
            let weights = gradient_band(&points, at, polar);
            (points, at, weights)
        }
    };
    normalized_or_nearest(weights, &points, at)
}

/// 1D weights: the bracketing pair cross-blends, the ends clamp.
pub fn weights_1d(thresholds: &[f32], x: f32) -> Vec<f32> {
    let mut weights = vec![0.0; thresholds.len()];
    let mut order: Vec<usize> = (0..thresholds.len()).collect();
    order.sort_by(|&a, &b| thresholds[a].total_cmp(&thresholds[b]));
    let (Some(&first), Some(&last)) = (order.first(), order.last()) else {
        return weights;
    };
    if x <= thresholds[first] {
        weights[first] = 1.0;
    } else if x >= thresholds[last] {
        weights[last] = 1.0;
    } else if let Some(pair) = order
        .windows(2)
        .find(|p| x >= thresholds[p[0]] && x <= thresholds[p[1]])
    {
        let (a, b) = (pair[0], pair[1]);
        let u = (x - thresholds[a]) / (thresholds[b] - thresholds[a]);
        weights[a] = 1.0 - u;
        weights[b] = u;
    }
    weights
}

/// Gradient-band influence of each point at `at` (not yet normalized).
pub fn gradient_band(points: &[Vec2], at: Vec2, polar: bool) -> Vec<f32> {
    (0..points.len())
        .map(|i| {
            let mut influence = 1.0_f32;
            for (j, &other) in points.iter().enumerate() {
                if i == j {
                    continue;
                }
                let (edge, to_at) = if polar {
                    polar_pair(points[i], other, at)
                } else {
                    (other - points[i], at - points[i])
                };
                let len2 = edge.length_squared();
                if len2 > EPSILON {
                    influence = influence.min(1.0 - to_at.dot(edge) / len2);
                }
            }
            influence.max(0.0)
        })
        .collect()
}

/// The vectors `p_i → p_j` and `p_i → at` in Johansen's polar space: x is the
/// magnitude difference over the pair's mean magnitude, y the signed angle from
/// `p_i` (scaled by [`ANGLE_WEIGHT`]). A child at the origin has no direction,
/// so a pair involving one blends on magnitude alone.
fn polar_pair(from: Vec2, to: Vec2, at: Vec2) -> (Vec2, Vec2) {
    let (m_from, m_to, m_at) = (from.length(), to.length(), at.length());
    let mean = ((m_from + m_to) * 0.5).max(EPSILON);
    let (angle_to, angle_at) = if m_from > EPSILON && m_to > EPSILON {
        (signed_angle(from, to), signed_angle(from, at))
    } else {
        (0.0, 0.0)
    };
    (
        Vec2::new((m_to - m_from) / mean, angle_to * ANGLE_WEIGHT),
        Vec2::new((m_at - m_from) / mean, angle_at * ANGLE_WEIGHT),
    )
}

/// The counter-clockwise angle from `a` to `b`, in `(-π, π]`; 0 when `b` is zero.
fn signed_angle(a: Vec2, b: Vec2) -> f32 {
    a.perp_dot(b).atan2(a.dot(b))
}

/// Scale `weights` to sum to 1. When nothing has any weight (the point far
/// outside every child's band), the child nearest it plays alone, so a tree with
/// children always produces a pose.
fn normalized_or_nearest(mut weights: Vec<f32>, points: &[Vec2], at: Vec2) -> Vec<f32> {
    let total: f32 = weights.iter().sum();
    if total > EPSILON && total.is_finite() {
        weights.iter_mut().for_each(|w| *w /= total);
        return weights;
    }
    let nearest = (0..points.len()).min_by(|&a, &b| {
        let (da, db) = (
            points[a].distance_squared(at),
            points[b].distance_squared(at),
        );
        da.total_cmp(&db)
    });
    weights.iter_mut().for_each(|w| *w = 0.0);
    if let Some(i) = nearest {
        weights[i] = 1.0;
    }
    weights
}

#[cfg(test)]
#[path = "blend_tree_tests.rs"]
mod blend_tree_tests;
