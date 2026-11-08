//! src/app/animation/blend_tree.rs — blend-tree weights (#457, nested #1746).
//!
//! How much each child of a [`BlendTree`] plays, from the `Float` parameters it
//! reads. A tree's own weights always sum to 1 (or are all 0 for a childless
//! tree):
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
//! A child may be a nested tree (#1746): [`for_each_leaf`] walks down, each
//! leaf clip weighted by the product of the weights on its path, as Unity does,
//! so the leaves' weights still sum to 1. A child of weight 0 is skipped, its
//! whole subtree with it.
//!
//! Pure functions of (tree, parameter values) — no state, deterministic, and
//! no allocation: the walk runs every fixed step for every animator playing a
//! tree.

use glam::Vec2;

use crate::asset::animation_graph::{AnimationEvent, BlendTree, SyncMarker, MAX_TREE_DEPTH};
use crate::components::{AnimatorParameter, AnimatorParameters};

/// How strongly the angle counts against the speed difference in the polar
/// space. Johansen's value, and the one Unity's directional trees behave like.
const ANGLE_WEIGHT: f32 = 2.0;

/// Below this a length is zero (a child at the origin, two coincident points).
const EPSILON: f32 = 1e-6;

/// One leaf clip of a (possibly nested) blend tree, with its weight in the whole
/// tree: the product of the weights on its path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Leaf<'a> {
    pub clip: &'a str,
    pub events: &'a [AnimationEvent],
    /// The leaf clip's sync markers (#1896).
    pub sync_markers: &'a [SyncMarker],
    pub weight: f32,
}

/// Visit every leaf clip of `tree` with a positive weight, depth first in
/// authored order. The visited weights sum to 1 (for a tree whose every path
/// ends in a clip). A tree nested deeper than [`MAX_TREE_DEPTH`], which
/// validation refuses, contributes nothing.
pub fn for_each_leaf<'a>(
    tree: &'a BlendTree,
    parameters: &AnimatorParameters,
    visit: &mut impl FnMut(Leaf<'a>),
) {
    walk(tree, parameters, 1.0, 1, visit);
}

fn walk<'a>(
    tree: &'a BlendTree,
    parameters: &AnimatorParameters,
    scale: f32,
    depth: usize,
    visit: &mut impl FnMut(Leaf<'a>),
) {
    for ((motion, _), w) in tree.children().zip(child_weights(tree, parameters)) {
        let weight = scale * w;
        if weight <= 0.0 {
            continue;
        }
        match &motion.blend_tree {
            Some(nested) if depth < MAX_TREE_DEPTH => {
                walk(nested, parameters, weight, depth + 1, visit)
            }
            Some(_) => {}
            None => visit(Leaf {
                clip: &motion.clip,
                events: &motion.events,
                sync_markers: &motion.sync_markers,
                weight,
            }),
        }
    }
}

/// How many children's raw weights [`child_weights`] keeps on the stack, so
/// each is computed once; a wider tree recomputes the ones past this.
const INLINE_CHILDREN: usize = 32;

/// Each direct child's weight within `tree`, in authored order. A parameter
/// that is unset or not a `Float` reads 0. When nothing has any weight (the
/// point far outside every child's band), the child nearest it plays alone, so
/// a tree with children always produces a pose.
pub fn child_weights<'a>(
    tree: &'a BlendTree,
    parameters: &AnimatorParameters,
) -> impl Iterator<Item = f32> + 'a {
    let at = sample_point(tree, parameters);
    let bracket = bracket_1d(tree, at.x);
    let raw = move |i: usize| match tree {
        BlendTree::Simple1D { .. } => match bracket {
            Some((a, b, _)) if a == b => f32::from(u8::from(i == a)),
            Some((a, _, u)) if i == a => 1.0 - u,
            Some((_, b, u)) if i == b => u,
            _ => 0.0,
        },
        BlendTree::FreeformDirectional2D { .. } => influence(tree, i, at, true),
        BlendTree::FreeformCartesian2D { .. } => influence(tree, i, at, false),
    };
    let mut kept = [0.0_f32; INLINE_CHILDREN];
    let mut total = 0.0_f32;
    for i in 0..tree.len() {
        let w = raw(i);
        if let Some(slot) = kept.get_mut(i) {
            *slot = w;
        }
        total += w;
    }
    let fallback = (!(total > EPSILON && total.is_finite())).then(|| nearest(tree, at));
    (0..tree.len()).map(move |i| match fallback {
        None => kept.get(i).copied().unwrap_or_else(|| raw(i)) / total,
        Some(n) => f32::from(u8::from(n == Some(i))),
    })
}

/// The leaf whose animation events a tree fires (#459, #1746): from the top,
/// the heaviest child at each level (the first on a tie), counting only the
/// children with a leaf beneath them that `resolves` (a clip the mesh has).
/// Its `weight` is its path's product, as in [`for_each_leaf`]. `None` when no
/// weighted leaf resolves.
pub fn dominant_leaf<'a>(
    tree: &'a BlendTree,
    parameters: &AnimatorParameters,
    resolves: &impl Fn(&str) -> bool,
) -> Option<Leaf<'a>> {
    dominant(tree, parameters, 1.0, 1, resolves)
}

fn dominant<'a>(
    tree: &'a BlendTree,
    parameters: &AnimatorParameters,
    scale: f32,
    depth: usize,
    resolves: &impl Fn(&str) -> bool,
) -> Option<Leaf<'a>> {
    let mut best: Option<(f32, Leaf<'a>)> = None;
    for ((motion, _), w) in tree.children().zip(child_weights(tree, parameters)) {
        if w <= 0.0 || best.is_some_and(|(b, _)| w <= b) {
            continue;
        }
        let leaf = match &motion.blend_tree {
            Some(nested) if depth < MAX_TREE_DEPTH => {
                dominant(nested, parameters, scale * w, depth + 1, resolves)
            }
            Some(_) => None,
            None => resolves(&motion.clip).then_some(Leaf {
                clip: &motion.clip,
                events: &motion.events,
                sync_markers: &motion.sync_markers,
                weight: scale * w,
            }),
        };
        if let Some(leaf) = leaf {
            best = Some((w, leaf));
        }
    }
    best.map(|(_, leaf)| leaf)
}

/// Where the parameters put the tree's sample point (1D on the x axis).
fn sample_point(tree: &BlendTree, parameters: &AnimatorParameters) -> Vec2 {
    let read = |name: &str| match parameters.get(name) {
        Some(AnimatorParameter::Float(v)) if v.is_finite() => *v,
        _ => 0.0,
    };
    match tree {
        BlendTree::Simple1D { parameter, .. } => Vec2::new(read(parameter), 0.0),
        BlendTree::FreeformDirectional2D {
            parameter_x,
            parameter_y,
            ..
        }
        | BlendTree::FreeformCartesian2D {
            parameter_x,
            parameter_y,
            ..
        } => Vec2::new(read(parameter_x), read(parameter_y)),
    }
}

/// A 1D tree's bracketing pair at `x`: the children `(a, b)` it blends and how
/// far from `a` to `b` it sits. Past either end both are the end child. `None`
/// for a 2D or childless tree. Ties (which validation refuses) resolve by
/// authored order: the earlier child on the low side, the later on the high.
fn bracket_1d(tree: &BlendTree, x: f32) -> Option<(usize, usize, f32)> {
    let BlendTree::Simple1D { children, .. } = tree else {
        return None;
    };
    let t = |i: usize| children[i].threshold;
    let indices = 0..children.len();
    let first = indices.clone().min_by(|&a, &b| t(a).total_cmp(&t(b)))?;
    let last = indices.clone().max_by(|&a, &b| t(a).total_cmp(&t(b)))?;
    if x <= t(first) {
        return Some((first, first, 0.0));
    }
    if x >= t(last) {
        return Some((last, last, 0.0));
    }
    let below = indices.clone().filter(|&i| t(i) < x);
    let a = below.max_by(|&a, &b| t(a).total_cmp(&t(b)))?;
    let b = indices
        .filter(|&i| t(i) >= x)
        .min_by(|&a, &b| t(a).total_cmp(&t(b)))?;
    Some((a, b, (x - t(a)) / (t(b) - t(a))))
}

/// Gradient-band influence of child `i` at `at` (not yet normalized).
fn influence(tree: &BlendTree, i: usize, at: Vec2, polar: bool) -> f32 {
    let point = |j: usize| tree.child(j).map_or(Vec2::ZERO, |(_, p)| Vec2::from(p));
    band(tree.len(), point, i, at, polar)
}

/// Gradient-band influence of point `i` of `count` at `at`: the smallest
/// fall-off toward any other point, clamped at 0.
fn band(count: usize, point: impl Fn(usize) -> Vec2, i: usize, at: Vec2, polar: bool) -> f32 {
    let mut influence = 1.0_f32;
    for j in (0..count).filter(|&j| j != i) {
        let (edge, to_at) = if polar {
            polar_pair(point(i), point(j), at)
        } else {
            (point(j) - point(i), at - point(i))
        };
        let len2 = edge.length_squared();
        if len2 > EPSILON {
            influence = influence.min(1.0 - to_at.dot(edge) / len2);
        }
    }
    influence.max(0.0)
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

/// The child nearest `at` (the first on a tie), `None` for a childless tree.
fn nearest(tree: &BlendTree, at: Vec2) -> Option<usize> {
    let distance = |i: usize| {
        tree.child(i)
            .map_or(f32::INFINITY, |(_, p)| Vec2::from(p).distance_squared(at))
    };
    (0..tree.len()).min_by(|&a, &b| distance(a).total_cmp(&distance(b)))
}

#[cfg(test)]
#[path = "blend_tree_tests.rs"]
mod blend_tree_tests;
#[cfg(test)]
#[path = "nested_tests.rs"]
pub(super) mod nested_tests;
