//! Events stay exact through a synced fade (#1896): a following incoming
//! motion crosses its own events once each, a re-match back and forth over an
//! event fires it once, and two runs give bit-identical poses.

use super::super::{sample_pose, step};
use super::rig::*;
use crate::asset::animation_graph::{AnimationEvent, AnimationGraph};
use crate::components::AnimatorComponent;

/// Locomotion with Run firing `Plant` at 0.2 s.
fn planted() -> AnimationGraph {
    let mut graph = locomotion(true);
    graph.base.nodes[1].events = vec![AnimationEvent {
        time: 0.2,
        name: "Plant".to_string(),
    }];
    graph
}

fn into_run(graph: &AnimationGraph, walk: f32, fade: f32) -> AnimatorComponent {
    let mut anim = bound(graph);
    anim.base.time = walk;
    anim.enter_node(graph.node("Run").unwrap(), fade);
    anim
}

fn plants(graph: &AnimationGraph, anim: &mut AnimatorComponent, steps: usize) -> usize {
    let mesh = rig_mesh();
    (0..steps)
        .flat_map(|_| step(anim, Some(&mesh), Some(graph), DT).events)
        .filter(|e| e.name == "Plant")
        .count()
}

#[test]
fn a_following_motion_fires_each_crossing_once() {
    let graph = planted();
    // Walk at FootL puts Run at 0.09; through the 0.5 s fade Walk reaches
    // FootR and Run its 0.39, crossing Plant once.
    let mut anim = into_run(&graph, 0.0, 0.5);
    assert_eq!(plants(&graph, &mut anim, 30), 1);
    assert!(!anim.base.is_crossfading());
}

#[test]
fn a_rematch_back_and_forth_over_an_event_fires_it_once() {
    let graph = planted();
    let mut anim = into_run(&graph, 0.0, 2.0);
    let mut fired = plants(&graph, &mut anim, 12); // Run passes 0.2
    assert_eq!(fired, 1);
    // Walk (the leader) jumps back: Run is re-matched back over Plant...
    anim.base.previous_time -= 0.25;
    fired += plants(&graph, &mut anim, 1);
    assert!(
        anim.base.time < 0.2,
        "re-matched back to {}",
        anim.base.time
    );
    // ...and forwards over it again as Walk plays on: still once in all.
    fired += plants(&graph, &mut anim, 16);
    assert!(anim.base.time > 0.2, "{}", anim.base.time);
    assert_eq!(fired, 1);
    // Past the travel it owed, the next lap fires again.
    fired += plants(&graph, &mut anim, 80);
    assert!(fired >= 2, "{fired}");
}

#[test]
fn travel_owed_back_stays_silent_once_the_incoming_motion_leads() {
    let graph = planted();
    // A 0.6 s fade: Run leads from step 18, re-crossing Plant after that.
    let mut anim = into_run(&graph, 0.0, 0.6);
    let mut fired = plants(&graph, &mut anim, 12);
    anim.base.previous_time -= 0.25; // Walk jumps back; Run is re-matched
    fired += plants(&graph, &mut anim, 1);
    assert!(anim.base.time < 0.2, "{}", anim.base.time);
    let mut crossed_leading = false;
    for _ in 0..24 {
        let below = anim.base.time < 0.2;
        fired += plants(&graph, &mut anim, 1);
        let leads = anim.base.crossfade_weight() >= 0.5;
        crossed_leading |= below && anim.base.time >= 0.2 && leads;
    }
    assert!(crossed_leading, "Run re-crossed Plant leading");
    assert_eq!(fired, 1);
}

#[test]
fn two_runs_give_bit_identical_poses() {
    let graph = planted();
    let mesh = rig_mesh();
    let run = || {
        let mut anim = into_run(&graph, 0.3, 0.5);
        let mut poses = Vec::new();
        for _ in 0..45 {
            step(&mut anim, Some(&mesh), Some(&graph), DT);
            poses.push(format!(
                "{:?}",
                sample_pose(&anim, &mesh, Some(&graph)).unwrap().1
            ));
        }
        poses
    };
    assert_eq!(run(), run());
}
