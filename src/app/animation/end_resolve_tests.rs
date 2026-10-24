//! `End` and the markers around it stay single-shot across a clip's length
//! coming and going (#1386): a clip that stops resolving for a step doesn't end
//! again, the step a late clip resolves replays no marker, and a reverse push
//! that can't leave a zero-length clip's end doesn't re-arm it.

use super::advance;
use super::end_tests::{clip_mesh, ends, rig, DT};
use super::graph::step_graph;
use super::test_rig::{graph, node};
use crate::asset::animation_graph::{AnimationEvent, AnimationGraph, GraphNode};
use crate::components::AnimatorComponent;

/// Non-looping clip `A` on a node with a `Tail` marker at `at` seconds.
fn marked(at: f32) -> (AnimationGraph, AnimatorComponent) {
    let a = GraphNode {
        events: vec![AnimationEvent {
            time: at,
            name: "Tail".to_string(),
        }],
        is_loop: false,
        ..node("A", "A")
    };
    let graph = graph(vec![a], vec![]);
    let mut anim = AnimatorComponent::default();
    step_graph(&mut anim, &graph);
    (graph, anim)
}

#[test]
fn a_finished_clip_that_stops_resolving_for_a_step_does_not_end_again() {
    let (graph, mut anim) = marked(1.0);
    let resolved = clip_mesh("A", 1.0);
    assert_eq!(ends(&mut anim, &resolved, &graph, 15), 1);
    assert!(anim.base.ended);
    // One step where `A` isn't on the mesh: its length is unknown.
    ends(&mut anim, &clip_mesh("B", 1.0), &graph, 1);
    assert!(anim.base.ended, "an unknown length leaves `ended` alone");
    let back = advance(&mut anim, Some(&resolved), Some(&graph), DT);
    assert!(back.is_empty(), "no End, no end marker: {back:?}");
}

#[test]
fn the_late_resolve_step_replays_no_marker_the_unresolved_run_crossed() {
    let (graph, mut anim) = marked(1.0);
    // Unresolved, the playhead runs past 1.0 and fires `Tail` at its raw time.
    let early: Vec<_> = (0..15)
        .flat_map(|_| advance(&mut anim, Some(&clip_mesh("B", 1.0)), Some(&graph), DT))
        .map(|e| e.name)
        .collect();
    assert_eq!(early, ["Tail"]);
    let step = advance(&mut anim, Some(&clip_mesh("A", 1.0)), Some(&graph), DT);
    let names: Vec<_> = step.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, [super::END_EVENT], "End once, `Tail` not replayed");
}

#[test]
fn a_reverse_push_that_cannot_leave_the_end_keeps_it_ended() {
    let (graph, mut anim) = rig(false);
    let pose = clip_mesh("A", 0.0);
    assert_eq!(ends(&mut anim, &pose, &graph, 1), 1);
    anim.speed = -1.0;
    assert_eq!(ends(&mut anim, &pose, &graph, 3), 0);
    assert!(anim.base.ended, "time 0 is still the end");
    anim.speed = 1.0;
    assert_eq!(ends(&mut anim, &pose, &graph, 3), 0, "no second End");
}
