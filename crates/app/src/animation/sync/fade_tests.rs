//! Synced crossfades (#1896): entering a node of the same group starts at the
//! outgoing phase, the two playheads stay matched through the fade, and a node
//! outside any group crossfades exactly as before.

use super::super::step;
use super::rig::*;

#[test]
fn walk_to_run_in_one_group_starts_run_at_walks_phase() {
    let (mesh, graph) = (rig_mesh(), locomotion(true));
    let mut anim = bound(&graph);
    anim.base.time = 0.3; // 60 % of the way from FootL to FootR
    anim.enter_node(graph.node("Run").unwrap(), 0.5);
    step(&mut anim, Some(&mesh), Some(&graph), 0.0);
    let run = anim.base.time;
    assert!(
        (run - (0.09 + 0.6 * 0.3)).abs() < 1e-5,
        "Run placed at {run}"
    );
    assert_same(walk_at(anim.base.previous_time), run_at(run), "start");
}

#[test]
fn a_half_second_fade_keeps_both_playheads_matched_every_step() {
    let (mesh, graph) = (rig_mesh(), locomotion(true));
    let mut anim = bound(&graph);
    anim.base.time = 0.7;
    anim.enter_node(graph.node("Run").unwrap(), 0.5);
    let mut steps = 0;
    while anim.base.is_crossfading() {
        step(&mut anim, Some(&mesh), Some(&graph), DT);
        if anim.base.is_crossfading() {
            let walk = anim.base.previous_time.rem_euclid(1.0);
            assert_same(
                walk_at(walk),
                run_at(anim.base.time),
                &format!("step {steps}"),
            );
        }
        steps += 1;
    }
    assert_eq!(steps, 30, "the fade ran its 0.5 s");
}

#[test]
fn nodes_in_no_group_crossfade_from_the_start_as_before() {
    let (mesh, graph) = (rig_mesh(), locomotion(false));
    let mut anim = bound(&graph);
    anim.base.time = 0.3;
    anim.enter_node(graph.node("Run").unwrap(), 0.5);
    step(&mut anim, Some(&mesh), Some(&graph), DT);
    assert!(
        (anim.base.time - DT).abs() < 1e-6,
        "Run starts at 0: {}",
        anim.base.time
    );
    assert!((anim.base.previous_time - (0.3 + DT)).abs() < 1e-6);
    assert!(anim.base.sync.is_idle(), "{:?}", anim.base.sync);
}

#[test]
fn a_hard_cut_into_the_group_still_lands_on_the_phase() {
    let (mesh, graph) = (rig_mesh(), locomotion(true));
    let mut anim = bound(&graph);
    anim.base.time = 0.75; // halfway from FootR to the next FootL
    anim.enter_node(graph.node("Run").unwrap(), 0.0);
    step(&mut anim, Some(&mesh), Some(&graph), 0.0);
    assert_same(walk_at(0.75), run_at(anim.base.time), "cut");
}

#[test]
fn a_non_looping_follower_holds_at_its_end() {
    let (mesh, mut graph) = (rig_mesh(), locomotion(true));
    graph.base.nodes[1].is_loop = false;
    graph.base.nodes[1].sync_markers.clear(); // follows by normalized time
    let mut anim = bound(&graph);
    anim.base.time = 0.6;
    anim.enter_node(graph.node("Run").unwrap(), 4.0);
    // Walk leads for the first 2 s: it wraps twice, Run reaches its end once.
    for _ in 0..90 {
        step(&mut anim, Some(&mesh), Some(&graph), DT);
    }
    assert!(
        (anim.base.time - RUN).abs() < 1e-6,
        "held at {}",
        anim.base.time
    );
    assert!(anim.base.ended);
}
