//! Nested blend trees' weights (#1746): leaf weights multiply down the tree and
//! sum to 1, a zero-weight subtree is skipped whole, the depth cap holds, and
//! the nearest-child fallback.

use std::collections::BTreeMap;

use super::blend_tree_tests::{child_2d, tree_1d, tree_weights};
use super::*;
use rusty_asset::animation_graph::{BlendChild2D, ChildMotion};

fn params(values: &[(&str, f32)]) -> AnimatorParameters {
    let float = |&(name, v): &(&str, f32)| (name.to_string(), AnimatorParameter::Float(v));
    values.iter().map(float).collect()
}

/// A directional tree over (`vx`, `vy`): forward plays `forward`, back, left
/// and right play clips.
pub(in crate::animation) fn directional(forward: ChildMotion) -> BlendTree {
    let at = |motion, position| BlendChild2D { motion, position };
    BlendTree::FreeformDirectional2D {
        parameter_x: "vx".into(),
        parameter_y: "vy".into(),
        children: vec![
            at(forward, [0.0, 1.0]),
            at(ChildMotion::clip("Back"), [0.0, -1.0]),
            at(ChildMotion::clip("Left"), [-1.0, 0.0]),
            at(ChildMotion::clip("Right"), [1.0, 0.0]),
        ],
    }
}

/// A 1D walk (threshold 0) / run (threshold 1) tree over `x`.
fn walk_run() -> BlendTree {
    let mut tree = tree_1d(&[0.0, 1.0]);
    if let BlendTree::Simple1D { children, .. } = &mut tree {
        children[0].motion = ChildMotion::clip("Walk");
        children[1].motion = ChildMotion::clip("Run");
    }
    tree
}

fn leaves<'a>(tree: &'a BlendTree, parameters: &AnimatorParameters) -> Vec<Leaf<'a>> {
    let mut out = Vec::new();
    for_each_leaf(tree, parameters, &mut |leaf| out.push(leaf));
    out
}

#[test]
fn leaf_weights_multiply_down_the_tree_and_sum_to_one() {
    let nested = directional(ChildMotion::tree(walk_run()));
    for (vx, vy, x) in [
        (0.0, 1.0, 0.25),
        (0.4, 0.6, 0.5),
        (-0.3, 0.2, 1.0),
        (0.2, -0.9, 0.7),
    ] {
        let p = params(&[("vx", vx), ("vy", vy), ("x", x)]);
        let top = tree_weights(&directional(ChildMotion::clip("Fwd")), &p);
        let inner = tree_weights(&walk_run(), &p);
        let got = leaves(&nested, &p);
        let total: f32 = got.iter().map(|l| l.weight).sum();
        assert!(
            (total - 1.0).abs() < 1e-5,
            "({vx}, {vy}, {x}) sums to {total}"
        );
        let weight = |clip: &str| {
            got.iter()
                .find(|l| l.clip == clip)
                .map_or(0.0, |l| l.weight)
        };
        let want = [top[0] * inner[0], top[0] * inner[1], top[1], top[2], top[3]];
        for (clip, want) in ["Walk", "Run", "Back", "Left", "Right"].iter().zip(want) {
            assert!(
                (weight(clip) - want).abs() < 1e-6,
                "{clip}: {}",
                weight(clip)
            );
        }
    }
}

#[test]
fn a_zero_weight_subtree_is_skipped_and_leaves_come_depth_first() {
    let nested = directional(ChildMotion::tree(walk_run()));
    // Straight back: the forward subtree weighs nothing, so none of it is visited.
    let back = leaves(&nested, &params(&[("vy", -1.0), ("x", 0.5)]));
    assert_eq!(back.iter().map(|l| l.clip).collect::<Vec<_>>(), ["Back"]);
    // Between forward and right: the subtree's leaves, then the right clip.
    let order = leaves(&nested, &params(&[("vx", 0.5), ("vy", 0.5), ("x", 0.5)]));
    let clips: Vec<&str> = order.iter().map(|l| l.clip).collect();
    assert_eq!(clips, ["Walk", "Run", "Right"]);
}

#[test]
fn trees_nested_past_the_depth_cap_contribute_nothing() {
    let chain = |depth: usize| {
        let mut tree = tree_1d(&[0.0]);
        for _ in 1..depth {
            let mut outer = tree_1d(&[0.0]);
            if let BlendTree::Simple1D { children, .. } = &mut outer {
                children[0].motion = ChildMotion::tree(tree);
            }
            tree = outer;
        }
        tree
    };
    let none = AnimatorParameters::new();
    assert_eq!(leaves(&chain(MAX_TREE_DEPTH), &none).len(), 1);
    assert!(leaves(&chain(MAX_TREE_DEPTH + 1), &none).is_empty());
}

#[test]
fn unset_parameters_read_zero_and_a_far_point_falls_back_to_the_nearest() {
    assert_eq!(
        tree_weights(&tree_1d(&[2.0, 0.0]), &BTreeMap::new()),
        [0.0, 1.0]
    );
    let cartesian = BlendTree::FreeformCartesian2D {
        parameter_x: "x".into(),
        parameter_y: "y".into(),
        children: vec![child_2d([0.0, 0.0]), child_2d([1.0, 0.0])],
    };
    assert_eq!(tree_weights(&cartesian, &params(&[("x", 5.0)])), [0.0, 1.0]);
}
