//! Root motion through a synced fade out of a marker-synced tree (#1896): a
//! change of the tree's leader mid-fade moves no root.

use super::super::step;
use super::rig::*;
use crate::asset::animation_graph::AnimationGraph;
use crate::components::AnimatorParameter;

/// The Walk/Run tree and a Run node, both in `Locomotion`.
fn tree_and_run() -> AnimationGraph {
    let mut loco = tree(walk_marks(), run_marks());
    loco.sync_group = Some("Locomotion".to_string());
    super::super::test_rig::graph(
        vec![loco, grouped("Run", Some("Locomotion"), run_marks())],
        Vec::new(),
    )
}

#[test]
fn a_leader_change_in_the_fading_tree_moves_no_root() {
    let (mesh, graph) = (hips_mesh(), tree_and_run());
    let mut anim = bound(&graph);
    let speed = |anim: &mut crate::components::AnimatorComponent, v| {
        let value = AnimatorParameter::Float(v);
        anim.parameters.insert("speed".to_string(), value);
    };
    speed(&mut anim, 0.4); // Walk leads the tree
    let mut roots = Vec::new();
    let mut go = |anim: &mut _, n| {
        for _ in 0..n {
            let root = step(anim, Some(&mesh), Some(&graph), DT).root;
            roots.push(root.unwrap_or_default().position.x);
        }
    };
    go(&mut anim, 13);
    anim.enter_node(graph.node("Run").unwrap(), 1.0);
    go(&mut anim, 10);
    assert!(anim.base.sync.following && anim.base.is_crossfading());
    speed(&mut anim, 0.6); // Run leads the fading tree now
    go(&mut anim, 20);
    assert_eq!(anim.base.sync.previous_leader.as_deref(), Some("Run"));
    for (i, x) in roots.iter().enumerate() {
        assert!((0.0..0.04).contains(x), "step {i}: root {x}");
    }
}
