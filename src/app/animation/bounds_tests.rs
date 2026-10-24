//! One [`Bounds`](crate::components::Bounds) per playhead (#1362): the `loop_clip`
//! flag, not the graph node's `is_loop`, decides how a motion advances, samples
//! and fires its markers; a marker past a clamped clip's end fires on arrival;
//! and no step — huge, or NaN — leaves either playhead out of `f32` range.

use super::graph::step_graph;
use super::test_rig::{animator, graph, mesh, node, slide, HIPS};
use super::tree_tests::tree_node;
use super::{advance, sample_pose};
use crate::asset::animation_graph::{AnimationEvent, GraphNode};
use crate::components::{AnimatorComponent, Motion, Playback, Playhead};

#[test]
fn a_looping_tree_set_not_to_loop_holds_its_last_frame() {
    let mesh = mesh(vec![slide("Walk", &[HIPS], 0.0, 2.0, 1.0)]);
    // The node is authored looping; `SetLooping(false)` clears the flag.
    let graph = graph(vec![tree_node("Loco", &[("Walk", 0.0)])], vec![]);
    let mut anim = animator(Motion::Tree("Loco"), None, 0.0);
    anim.base.loop_clip = false;
    advance(&mut anim, Some(&mesh), Some(&graph), 1.5);
    assert_eq!(anim.base.time, 1.0, "clamped at the end");
    let pose = sample_pose(&anim, &mesh, Some(&graph)).unwrap().1;
    let x = pose[HIPS].unwrap().translation.x;
    assert!((x - 2.0).abs() < 1e-5, "the last frame, not phase 0: x {x}");
}

#[test]
fn a_marker_past_a_clamped_end_fires_once_on_arrival_whatever_the_step() {
    for dt in [0.1, 0.3, 0.7] {
        let late = AnimationEvent {
            time: 1.5,
            name: "Late".to_string(),
        };
        let a = GraphNode {
            is_loop: false,
            events: vec![late],
            ..node("A", "A")
        };
        let graph = graph(vec![a], vec![]);
        let mut anim = AnimatorComponent::default();
        step_graph(&mut anim, &graph);
        let mesh = mesh(vec![slide("A", &[HIPS], 0.0, 1.0, 1.0)]);
        let fired = (0..20)
            .flat_map(|_| advance(&mut anim, Some(&mesh), Some(&graph), dt))
            .filter(|e| e.name == "Late")
            .count();
        assert_eq!(fired, 1, "dt {dt}");
    }
}

#[test]
fn a_crossfade_keeps_its_outgoing_playhead_finite() {
    let mut playback = Playback::default();
    playback.play(Motion::Clip("A"));
    playback.crossfade(Motion::Clip("B"), 1e30);
    for _ in 0..20 {
        playback.advance(0.1, 3e38, Playhead::clip(1.0), 1.0);
    }
    assert!(
        playback.previous_time.is_finite(),
        "{}",
        playback.previous_time
    );
}

#[test]
fn a_nan_step_stands_still() {
    let mut playback = Playback::default();
    playback.play(Motion::Clip("A"));
    playback.time = 0.5;
    // A node's `speed: 0` times an infinite animator speed is NaN.
    playback.node_speed = 0.0;
    let span = playback
        .advance(0.1, f32::INFINITY, Playhead::clip(1.0), 1.0)
        .span;
    assert_eq!((playback.time, span), (0.5, (0.5, 0.5)));
}
