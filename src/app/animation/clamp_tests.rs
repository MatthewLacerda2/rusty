//! A non-looping motion's playhead clamps to `[0, length]` (#1362), the way
//! Unity's sampled pose does: reversing from past either end moves back on the
//! very next step, `End` fires once per forward pass, and no speed pushes the
//! playhead out of `f32` range.

use super::advance;
use super::graph::step_graph;
use super::test_rig::{graph, mesh, node, slide, HIPS};
use crate::asset::animation_graph::{AnimationGraph, GraphNode};
use crate::components::AnimatorComponent;

const DT: f32 = 0.1;

/// A one-second clip `A` on a node, looping or not, and an animator bound to it.
fn rig(is_loop: bool) -> (AnimationGraph, AnimatorComponent) {
    let a = GraphNode {
        is_loop,
        ..node("A", "A")
    };
    let graph = graph(vec![a], vec![]);
    let mut anim = AnimatorComponent::default();
    step_graph(&mut anim, &graph);
    (graph, anim)
}

/// Run `steps` fixed steps at `speed`; return the `End` events they fired.
fn run(graph: &AnimationGraph, anim: &mut AnimatorComponent, steps: usize, speed: f32) -> usize {
    anim.speed = speed;
    let mesh = mesh(vec![slide("A", &[HIPS], 0.0, 1.0, 1.0)]);
    (0..steps)
        .flat_map(|_| advance(anim, Some(&mesh), Some(graph), DT))
        .filter(|e| e.name == super::END_EVENT)
        .count()
}

#[test]
fn reversing_after_the_end_moves_back_at_once_and_end_fires_once_per_pass() {
    let (graph, mut anim) = rig(false);
    assert_eq!(run(&graph, &mut anim, 50, 1.0), 1, "5 s on a 1 s clip");
    assert_eq!(anim.base.time, 1.0, "the playhead stops at the end");
    assert_eq!(
        run(&graph, &mut anim, 1, -1.0),
        0,
        "leaving the end is no End"
    );
    assert!(
        (anim.base.time - 0.9).abs() < 1e-6,
        "back on the very next step"
    );
    assert_eq!(run(&graph, &mut anim, 20, -1.0), 0);
    assert_eq!(anim.base.time, 0.0, "the playhead stops at the start");
    assert_eq!(
        run(&graph, &mut anim, 20, 1.0),
        1,
        "a new pass ends once more"
    );
}

#[test]
fn reversing_from_the_start_then_forward_moves_at_once() {
    let (graph, mut anim) = rig(false);
    run(&graph, &mut anim, 20, -1.0);
    assert_eq!(anim.base.time, 0.0);
    run(&graph, &mut anim, 1, 1.0);
    assert!((anim.base.time - 0.1).abs() < 1e-6, "no held frame 0");
}

#[test]
fn an_end_reached_exactly_fires_once() {
    let (graph, mut anim) = rig(false);
    anim.base.time = 0.5;
    assert_eq!(
        run(&graph, &mut anim, 1, 5.0),
        1,
        "0.5 + 0.1 × 5 lands on 1.0"
    );
    assert_eq!(
        run(&graph, &mut anim, 5, 5.0),
        0,
        "resting on the end is silent"
    );
}

#[test]
fn a_huge_speed_keeps_the_playhead_finite() {
    for is_loop in [false, true] {
        let (graph, mut anim) = rig(is_loop);
        run(&graph, &mut anim, 200, 3e38);
        assert!(
            anim.base.time.is_finite(),
            "loop {is_loop}: {}",
            anim.base.time
        );
        run(&graph, &mut anim, 1, -1.0);
        assert!(anim.base.time.is_finite(), "loop {is_loop}");
    }
    let (graph, mut anim) = rig(false);
    run(&graph, &mut anim, 200, 3e38);
    run(&graph, &mut anim, 1, -1.0);
    assert!(
        (anim.base.time - 0.9).abs() < 1e-6,
        "still reverses from the end"
    );
}
