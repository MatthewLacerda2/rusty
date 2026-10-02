//! Blend-tree motions (#457): the weighted children blend in TRS space at one
//! shared phase, the tree's playhead runs at its weighted cycle length, and a
//! crossfade can leave a tree.

use super::test_rig::*;
use super::*;
use crate::asset::animation_graph::{BlendChild1D, BlendTree, GraphNode};
use crate::components::Motion;

/// A looping 1D tree node over `speed`: `(clip, threshold)` children.
pub fn tree_node(name: &str, children: &[(&str, f32)]) -> GraphNode {
    let children = children
        .iter()
        .map(|&(clip, threshold)| BlendChild1D {
            clip: clip.to_string(),
            threshold,
            events: Vec::new(),
        })
        .collect();
    GraphNode {
        clip: String::new(),
        blend_tree: Some(BlendTree::Simple1D {
            parameter: "speed".to_string(),
            children,
        }),
        ..node(name, "")
    }
}

/// Walk slides the hips 0 → 2 over 1 s, Run 0 → 6 over 2 s; a 1D tree puts
/// them at speed 2 and 6.
fn locomotion() -> (MeshComponent, AnimationGraph) {
    let mesh = mesh(vec![
        slide("Walk", &[HIPS], 0.0, 2.0, 1.0),
        slide("Run", &[HIPS], 0.0, 6.0, 2.0),
        slide("Idle", &[HIPS], 0.0, 0.0, 1.0),
    ]);
    let tree = tree_node("Loco", &[("Walk", 2.0), ("Run", 6.0), ("Ghost", 9.0)]);
    (mesh, graph(vec![tree, node("Idle", "Idle")], Vec::new()))
}

fn hips_x(anim: &AnimatorComponent, mesh: &MeshComponent, graph: &AnimationGraph) -> f32 {
    let pose = sample_pose(anim, mesh, Some(graph)).unwrap().1;
    pose[HIPS].unwrap().translation.x
}

#[test]
fn children_blend_at_one_shared_phase() {
    let (mesh, graph) = locomotion();
    let mut anim = animator(Motion::Tree("Loco"), None, 0.5);
    // Phase 0.5 is 0.5 s into Walk (x = 1) and 1 s into Run (x = 3).
    for (speed, x) in [(0.0, 1.0), (2.0, 1.0), (4.0, 2.0), (6.0, 3.0)] {
        anim.set_float("speed", speed);
        let got = hips_x(&anim, &mesh, &graph);
        assert!((got - x).abs() < 1e-5, "speed {speed}: x {got}, want {x}");
    }
    // Only keyed joints are written, by any child.
    let pose = sample_pose(&anim, &mesh, Some(&graph)).unwrap().1;
    assert!(pose[SPINE].is_none() && pose[LEG].is_none());
}

#[test]
fn the_tree_playhead_runs_at_its_weighted_cycle_length_and_wraps() {
    let (mesh, graph) = locomotion();
    let mut anim = animator(Motion::Tree("Loco"), None, 0.0);
    anim.base.loop_clip = true;
    anim.set_float("speed", 4.0);
    // Half Walk (1 s), half Run (2 s): one cycle every 1.5 s.
    advance(&mut anim, Some(&mesh), Some(&graph), 0.75);
    assert!(
        (anim.base.time - 0.5).abs() < 1e-6,
        "phase {}",
        anim.base.time
    );
    advance(&mut anim, Some(&mesh), Some(&graph), 1.5);
    assert!(
        (anim.base.time - 0.5).abs() < 1e-5,
        "wrapped to {}",
        anim.base.time
    );
    // Without the graph the tree has no length: the playhead stands still.
    advance(&mut anim, Some(&mesh), None, 0.75);
    assert!((anim.base.time - 0.5).abs() < 1e-5);
}

#[test]
fn a_crossfade_out_of_a_tree_blends_the_tree_pose() {
    let (mesh, graph) = locomotion();
    let mut anim = animator(Motion::Tree("Loco"), None, 0.5);
    anim.set_float("speed", 4.0);
    anim.base.crossfade(Motion::Clip("Idle"), 1.0);
    anim.base.crossfade_elapsed = 0.5;
    assert_eq!(anim.base.previous_motion(), Some(Motion::Tree("Loco")));
    // Half the tree's x = 2 and half Idle's x = 0.
    assert!((hips_x(&anim, &mesh, &graph) - 1.0).abs() < 1e-5);
    // Playing a clip directly leaves the tree for good.
    anim.play("Walk".to_string());
    assert_eq!(anim.base.motion(), Motion::Clip("Walk"));
    assert_eq!(anim.base.current_tree, None);
}
