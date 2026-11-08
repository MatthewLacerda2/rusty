//! Marker sync inside a blend tree (#1896): a run planting 0.15 later than
//! the walk keeps both clips' feet together at every blend weight, a tree
//! without markers keeps today's normalized sync, and a change of leader
//! doesn't jump.

use super::super::motion::MotionContext;
use super::super::test_rig::*;
use super::super::{sample_pose, step};
use super::rig::*;
use crate::asset::animation_graph::AnimationGraph;
use crate::components::{AnimatorComponent, AnimatorParameter};

fn loco(marked: bool) -> AnimationGraph {
    let (walk, run) = if marked {
        (walk_marks(), run_marks())
    } else {
        (Vec::new(), Vec::new())
    };
    graph(vec![tree(walk, run)], Vec::new())
}

fn at_speed(graph: &AnimationGraph, speed: f32) -> AnimatorComponent {
    let mut anim = bound(graph);
    anim.parameters
        .insert("speed".to_string(), AnimatorParameter::Float(speed));
    anim
}

/// Each leaf's sample time at the tree's current phase.
fn leaf_times(graph: &AnimationGraph, anim: &AnimatorComponent) -> (f32, f32) {
    let mesh = rig_mesh();
    let ctx = MotionContext {
        clips: &mesh.clips,
        machine: Some(&graph.base),
        parameters: &anim.parameters,
    };
    let phase = anim.base.time;
    let sync = ctx.tree_sync("Loco", phase, true);
    let time = |clip: usize, marks: &[_]| {
        let clip = &mesh.clips[clip];
        sync.as_ref()
            .map_or(phase * clip.duration, |s| s.leaf_time(clip, marks, phase))
    };
    (time(0, &walk_marks()), time(1, &run_marks()))
}

#[test]
fn feet_stay_together_at_every_blend_weight() {
    let (mesh, graph) = (rig_mesh(), loco(true));
    for speed in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let mut anim = at_speed(&graph, speed);
        for i in 0..80 {
            step(&mut anim, Some(&mesh), Some(&graph), DT);
            let (walk, run) = leaf_times(&graph, &anim);
            assert_same(
                walk_at(walk),
                run_at(run),
                &format!("speed {speed} step {i}"),
            );
        }
    }
}

#[test]
fn the_pose_samples_each_leaf_at_its_matched_time() {
    let (mesh, graph) = (rig_mesh(), loco(true));
    let mut anim = at_speed(&graph, 0.5);
    anim.base.time = 0.0; // Walk (the leader on a tie) on FootL
    let (_, pose) = sample_pose(&anim, &mesh, Some(&graph)).unwrap();
    // Each leaf keys its own joint, blended half-and-half with the bind pose.
    let run_x = pose[LEG].unwrap().translation.x * 2.0;
    assert!(
        (run_x * RUN - 0.09).abs() < 1e-4,
        "Run on its FootL: {run_x}"
    );
    step(&mut anim, Some(&mesh), Some(&graph), 0.0);
    assert_eq!(anim.base.sync.leader.as_deref(), Some("Walk"));
}

#[test]
fn a_tree_without_markers_keeps_normalized_sync() {
    let (mesh, graph) = (rig_mesh(), loco(false));
    let mut anim = at_speed(&graph, 0.5);
    for _ in 0..20 {
        step(&mut anim, Some(&mesh), Some(&graph), DT);
    }
    let (walk, run) = leaf_times(&graph, &anim);
    assert!((walk / WALK - run / RUN).abs() < 1e-6, "{walk} {run}");
    assert!(anim.base.sync.is_idle());
}

#[test]
fn a_change_of_leader_carries_the_phase() {
    let (mesh, graph) = (rig_mesh(), loco(true));
    let mut anim = at_speed(&graph, 0.4); // Walk leads
    for _ in 0..13 {
        step(&mut anim, Some(&mesh), Some(&graph), DT);
    }
    let (_, run_before) = leaf_times(&graph, &anim);
    anim.parameters
        .insert("speed".to_string(), AnimatorParameter::Float(0.6)); // Run leads
    step(&mut anim, Some(&mesh), Some(&graph), 0.0);
    assert_eq!(anim.base.sync.leader.as_deref(), Some("Run"));
    let (_, run_after) = leaf_times(&graph, &anim);
    assert!(
        (run_after - run_before).abs() < 1e-5,
        "{run_before} -> {run_after}"
    );
}
