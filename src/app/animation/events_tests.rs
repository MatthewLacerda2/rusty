//! Animation events (#459): each crossing fires exactly once — across loop
//! wraps, skipped-over markers and speed 0 — only from the current motion, the
//! dominant blend-tree child and a weighted layer, in time order.

use super::super::graph::step_graph;
use super::super::test_rig::*;
use super::super::tree_tests::tree_node;
use super::super::*;
use crate::asset::animation_graph::{AnimationEvent, BlendTree, GraphNode, LayerBlending};

fn marks(events: &[(&str, f32)]) -> Vec<AnimationEvent> {
    let event = |&(name, time): &(&str, f32)| AnimationEvent {
        time,
        name: name.to_string(),
    };
    events.iter().map(event).collect()
}

/// A node playing `clip` with `events`, looping or not.
fn marked(name: &str, clip: &str, is_loop: bool, events: &[(&str, f32)]) -> GraphNode {
    GraphNode {
        is_loop,
        events: marks(events),
        ..node(name, clip)
    }
}

/// One-second clips A and B, and an animator bound to `graph`'s entry.
fn rig(graph: &AnimationGraph) -> (MeshComponent, AnimatorComponent) {
    let mesh = mesh(vec![
        slide("A", &[HIPS], 0.0, 1.0, 1.0),
        slide("B", &[SPINE], 0.0, 1.0, 1.0),
        slide("Run", &[HIPS], 0.0, 1.0, 2.0),
    ]);
    let mut anim = AnimatorComponent::default();
    step_graph(&mut anim, graph);
    (mesh, anim)
}

/// The event names `steps` fixed steps of `dt` fire, in order.
fn run(graph: &AnimationGraph, anim: &mut AnimatorComponent, steps: usize, dt: f32) -> Vec<String> {
    let mesh = rig(graph).0;
    (0..steps)
        .flat_map(|_| advance(anim, Some(&mesh), Some(graph), dt))
        .map(|e| e.name)
        .collect()
}

#[test]
fn a_looping_marker_fires_once_per_loop() {
    let graph = graph(
        vec![marked("A", "A", true, &[("Start", 0.0), ("Step", 0.25)])],
        vec![],
    );
    let mut anim = rig(&graph).1;
    let fired = run(&graph, &mut anim, 170, 1.0 / 60.0); // 2.83 s
    let count = |n: &str| fired.iter().filter(|f| *f == n).count();
    assert_eq!((count("Start"), count("Step")), (3, 3), "{fired:?}");
}

#[test]
fn a_skipped_over_marker_fires_once_and_a_double_loop_twice() {
    let graph = graph(vec![marked("A", "A", false, &[("Hit", 0.5)])], vec![]);
    let mut anim = rig(&graph).1;
    assert_eq!(run(&graph, &mut anim, 1, 0.9), ["Hit"]);
    assert_eq!(run(&graph, &mut anim, 1, 0.9), [END_EVENT]);
    assert!(run(&graph, &mut anim, 5, 0.9).is_empty(), "End fires once");
    let looping = graph_of(marked("A", "A", true, &[("Hit", 0.5)]));
    let mut anim = rig(&looping).1;
    assert_eq!(run(&looping, &mut anim, 1, 2.5), ["Hit", "Hit"]);
}

fn graph_of(node: GraphNode) -> AnimationGraph {
    graph(vec![node], vec![])
}

#[test]
fn speed_zero_fires_nothing_even_on_a_marker() {
    let graph = graph_of(marked("A", "A", true, &[("Hit", 0.5)]));
    let mut anim = rig(&graph).1;
    anim.base.time = 0.5;
    anim.speed = 0.0;
    assert!(run(&graph, &mut anim, 10, 0.1).is_empty());
}

#[test]
fn events_in_one_step_fire_in_time_order_across_layers() {
    let base = marked("A", "A", true, &[("late", 0.6), ("early", 0.2)]);
    let upper = marked("B", "B", true, &[("middle", 0.4)]);
    let graph = graph(
        vec![base],
        vec![layer(LayerBlending::Override, &[], vec![upper])],
    );
    let mut anim = rig(&graph).1;
    assert_eq!(run(&graph, &mut anim, 1, 0.8), ["early", "middle", "late"]);
    anim.layers[0].weight = 0.0;
    assert_eq!(
        run(&graph, &mut anim, 1, 0.7),
        ["early"],
        "a weight-0 layer is silent"
    );
}

#[test]
fn a_clip_being_faded_out_is_silent_and_the_incoming_one_fires() {
    let a = marked("A", "A", true, &[("out", 0.5)]);
    let b = marked("B", "B", true, &[("in", 0.0)]);
    let graph = graph(vec![a, b.clone()], vec![]);
    let mut anim = rig(&graph).1;
    anim.base.time = 0.4;
    anim.enter_node(&b, 0.5); // A fades out across its 0.5 marker
    assert_eq!(run(&graph, &mut anim, 4, 0.1), ["in"]);
}

#[test]
fn a_blend_tree_fires_only_its_dominant_child() {
    let mut tree = tree_node("Loco", &[("A", 2.0), ("Run", 6.0)]);
    if let Some(BlendTree::Simple1D { children, .. }) = &mut tree.blend_tree {
        children[0].events = marks(&[("walkStep", 0.5)]);
        children[1].events = marks(&[("runStep", 1.0)]); // phase 0.5 of 2 s
    }
    let graph = graph_of(tree);
    let mut anim = rig(&graph).1;
    anim.set_float("speed", 3.0); // A at 0.75, Run at 0.25
    assert_eq!(run(&graph, &mut anim, 10, 0.1), ["walkStep"]);
    anim.set_float("speed", 5.0);
    assert_eq!(run(&graph, &mut anim, 14, 0.1), ["runStep"]);
}
