//! A non-looping outgoing motion following a looping incoming leader (#1896)
//! goes on to its end and holds there, never snapping back as the leader
//! wraps: neither its pose nor its root motion jumps.

use super::super::{sample_pose, step};
use super::rig::*;
use crate::asset::animation_graph::AnimationGraph;
use crate::components::{AnimatorComponent, MeshComponent};

use super::super::test_rig::LEG;

/// Locomotion with Run a non-looping start (WalkStart's role) following by
/// normalized time.
fn start_graph() -> AnimationGraph {
    let mut graph = locomotion(true);
    graph.base.nodes[1].is_loop = false;
    graph.base.nodes[1].sync_markers.clear();
    graph
}

/// Run at half way, fading into Walk over `fade` seconds.
fn run_into_walk(mesh: &MeshComponent, graph: &AnimationGraph, fade: f32) -> AnimatorComponent {
    let mut anim = bound(graph);
    anim.enter_node(graph.node("Run").unwrap(), 0.0);
    step(&mut anim, Some(mesh), Some(graph), 0.0);
    anim.base.time = 0.3;
    anim.enter_node(graph.node("Walk").unwrap(), fade);
    anim
}

#[test]
fn the_outgoing_start_holds_its_end_while_walk_leads_and_wraps() {
    let (mesh, graph) = (hips_mesh(), start_graph());
    let mut anim = run_into_walk(&mesh, &graph, 2.0);
    for i in 0..118 {
        let root = step(&mut anim, Some(&mesh), Some(&graph), DT).root;
        let x = root.unwrap_or_default().position.x;
        assert!(
            (-1e-4..=DT / RUN + 1e-4).contains(&x),
            "step {i}: root {x} (walk {}, run {})",
            anim.base.time,
            anim.base.previous_time
        );
        if anim.base.crossfade_weight() < 0.5 {
            continue; // Run leads, playing on to its end
        }
        assert!(anim.base.previous_time >= RUN, "step {i}: Run snapped back");
        let leg = sample_pose(&anim, &mesh, Some(&graph)).unwrap().1[LEG];
        let out = 1.0 - anim.base.crossfade_weight();
        let x = leg.unwrap().translation.x;
        assert!((x - out).abs() < 1e-4, "step {i}: Run's end pose: {x}");
    }
    assert!(anim.base.time > 0.9, "Walk led on: {}", anim.base.time);
}

#[test]
fn a_looping_walk_wraps_without_moving_the_held_start_back() {
    let (mesh, graph) = (hips_mesh(), start_graph());
    // Walk leads for the last 1.5 s of a 3 s fade, wrapping once.
    let mut anim = run_into_walk(&mesh, &graph, 3.0);
    let mut wrapped = false;
    while anim.base.is_crossfading() {
        let (last, leading) = (anim.base.time, anim.base.crossfade_weight() >= 0.5);
        let held = anim.base.previous_time;
        step(&mut anim, Some(&mesh), Some(&graph), DT);
        wrapped |= leading && anim.base.time < last;
        if leading && anim.base.is_crossfading() {
            assert!(anim.base.previous_time >= held.min(RUN), "moved back");
        }
    }
    assert!(wrapped, "Walk wrapped while it led");
}
