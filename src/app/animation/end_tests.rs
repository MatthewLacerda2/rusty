//! `End` fires once whenever a non-looping playhead first reaches its end
//! (#1386): also on the step a clip resolves with the playhead already past its
//! length, and on a zero-length clip's (a single-keyframe pose's) first step.

use super::advance;
use super::graph::step_graph;
use super::test_rig::{graph, mesh, node, slide, HIPS};
use crate::asset::animation_graph::{AnimationGraph, GraphNode};
use crate::components::{AnimatorComponent, MeshComponent};

const DT: f32 = 0.1;

/// Clip `A` on a node, looping or not, and an animator bound to it.
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

/// A mesh whose only clip, `name`, lasts `duration` seconds.
fn clip_mesh(name: &str, duration: f32) -> MeshComponent {
    mesh(vec![slide(name, &[HIPS], 0.0, 1.0, duration)])
}

/// The `End` events `steps` fixed steps over `mesh` fire.
fn ends(
    anim: &mut AnimatorComponent,
    mesh: &MeshComponent,
    graph: &AnimationGraph,
    steps: usize,
) -> usize {
    (0..steps)
        .flat_map(|_| advance(anim, Some(mesh), Some(graph), DT))
        .filter(|e| e.name == super::END_EVENT)
        .count()
}

#[test]
fn a_clip_resolving_past_its_end_fires_end_once_on_that_step() {
    for speed in [1.0, 0.0] {
        let (graph, mut anim) = rig(false);
        // `A` isn't on the mesh yet: its length is unknown, the playhead runs on.
        assert_eq!(ends(&mut anim, &clip_mesh("B", 1.0), &graph, 15), 0);
        assert!(anim.base.time > 1.0, "{}", anim.base.time);
        anim.speed = speed;
        let resolved = clip_mesh("A", 1.0);
        assert_eq!(ends(&mut anim, &resolved, &graph, 1), 1, "speed {speed}");
        assert_eq!(anim.base.time, 1.0, "clamped to the end");
        assert_eq!(
            ends(&mut anim, &resolved, &graph, 10),
            0,
            "once, not every step"
        );
    }
}

#[test]
fn a_zero_length_clip_ends_on_its_first_step() {
    let (graph, mut anim) = rig(false);
    let pose = clip_mesh("A", 0.0);
    assert_eq!(ends(&mut anim, &pose, &graph, 1), 1);
    assert_eq!(
        ends(&mut anim, &pose, &graph, 10),
        0,
        "once, not every step"
    );
    assert_eq!(anim.base.time, 0.0);
}

#[test]
fn a_zero_length_loop_never_ends() {
    let (graph, mut anim) = rig(true);
    assert_eq!(ends(&mut anim, &clip_mesh("A", 0.0), &graph, 10), 0);
}

#[test]
fn a_normal_clip_ends_exactly_once_whatever_the_step() {
    for speed in [0.3, 1.0, 7.0, 25.0] {
        let (graph, mut anim) = rig(false);
        anim.speed = speed;
        assert_eq!(
            ends(&mut anim, &clip_mesh("A", 1.0), &graph, 60),
            1,
            "speed {speed}"
        );
    }
}
