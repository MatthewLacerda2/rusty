//! Entering a node of the group (#1896) at the edges: before the clips load,
//! twice before a step, and into the node already playing.

use super::super::step;
use super::rig::*;

#[test]
fn a_placement_waits_for_the_clips_to_load() {
    let (mesh, graph) = (rig_mesh(), locomotion(true));
    let mut anim = bound(&graph);
    anim.base.time = 0.3;
    anim.enter_node(graph.node("Run").unwrap(), 0.0);
    step(&mut anim, None, Some(&graph), 0.0); // no mesh, no clips yet
    assert!(anim.base.sync.from.is_some(), "still pending");
    step(&mut anim, Some(&mesh), Some(&graph), 0.0);
    assert!((anim.base.time - 0.27).abs() < 1e-5, "{}", anim.base.time);
    assert!(anim.base.sync.from.is_none());
}

#[test]
fn two_entries_before_a_step_sync_both_motions_to_the_first() {
    let (mesh, graph) = (rig_mesh(), locomotion(true));
    let mut anim = bound(&graph);
    anim.base.time = 0.3; // Walk 60 % of the way FootL → FootR
    anim.enter_node(graph.node("Run").unwrap(), 0.5);
    anim.enter_node(graph.node("Walk").unwrap(), 0.5);
    step(&mut anim, Some(&mesh), Some(&graph), 0.0);
    // Walk back at its own phase, and the Run it fades out (never placed)
    // at that phase too, not its unplaced 0.
    assert!((anim.base.time - 0.3).abs() < 1e-5, "{}", anim.base.time);
    let run = anim.base.previous_time;
    assert!((run - 0.27).abs() < 1e-5, "Run placed at {run}");
    assert_same(walk_at(anim.base.time), run_at(run), "both");
}

#[test]
fn re_entering_the_playing_node_restarts_it() {
    let (mesh, graph) = (rig_mesh(), locomotion(true));
    let mut anim = bound(&graph);
    anim.base.time = 0.3;
    anim.enter_node(graph.node("Walk").unwrap(), 0.5);
    step(&mut anim, Some(&mesh), Some(&graph), 0.0);
    assert_eq!(anim.base.time, 0.0);
    assert!(!anim.base.sync.following);
    assert!(anim.base.sync.from.is_none());
}
