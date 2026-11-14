//! Nested blend trees play (#1746): a 2D tree whose forward child is a 1D
//! walk/run tree gives the pose of its hand-flattened leaves, keeps one shared
//! phase at its weighted cycle length, and fires only its dominant leaf's events.

use super::blend_tree::child_weights;
use super::blend_tree::nested_tests::directional;
use super::test_rig::*;
use super::*;
use rusty_asset::animation_graph::{
    AnimationEvent, BlendChild1D, BlendTree, ChildMotion, GraphNode, ParameterDeclaration,
};
use rusty_components::{AnimatorParameter, Motion};

/// `(clip, end x, duration)`: each slides the hips from 0 to its end x.
pub(super) const CLIPS: [(&str, f32, f32); 5] = [
    ("Walk", 2.0, 1.0),
    ("Run", 6.0, 2.0),
    ("Back", -2.0, 1.2),
    ("Left", -1.0, 0.8),
    ("Right", 1.0, 0.9),
];

/// The 1D walk (speed 2) / run (speed 6) tree, Walk stepping at 0.5 s.
pub(super) fn walk_run() -> BlendTree {
    let mut walk = ChildMotion::clip("Walk");
    walk.events = vec![AnimationEvent {
        time: 0.5,
        name: "walkStep".into(),
    }];
    let children = vec![
        BlendChild1D {
            motion: walk,
            threshold: 2.0,
        },
        BlendChild1D {
            motion: ChildMotion::clip("Run"),
            threshold: 6.0,
        },
    ];
    BlendTree::Simple1D {
        parameter: "speed".into(),
        children,
    }
}

pub(super) fn nested() -> (MeshComponent, AnimationGraph) {
    let clips = CLIPS.iter().map(|&(n, x, d)| slide(n, &[HIPS], 0.0, x, d));
    let tree = GraphNode {
        clip: String::new(),
        blend_tree: Some(directional(ChildMotion::tree(walk_run()))),
        ..node("Loco", "")
    };
    let mut graph = graph(vec![tree], Vec::new());
    for axis in ["vx", "vy"] {
        graph
            .parameters
            .insert(axis.into(), ParameterDeclaration::Float(0.0));
    }
    assert_eq!(graph.validate(), Ok(()));
    (mesh(clips.collect()), graph)
}

/// Each leaf clip's weight, worked out level by level from single-level trees.
fn flattened(anim: &AnimatorComponent) -> Vec<(&'static str, f32)> {
    let top: Vec<f32> =
        child_weights(&directional(ChildMotion::clip("Walk")), &anim.parameters).collect();
    let inner: Vec<f32> = child_weights(&walk_run(), &anim.parameters).collect();
    let weights = [top[0] * inner[0], top[0] * inner[1], top[1], top[2], top[3]];
    CLIPS.iter().map(|c| c.0).zip(weights).collect()
}

pub(super) fn set(anim: &mut AnimatorComponent, values: [f32; 3]) {
    for (name, v) in ["vx", "vy", "speed"].into_iter().zip(values) {
        anim.parameters
            .insert(name.into(), AnimatorParameter::Float(v));
    }
}

const SWEEP: [[f32; 3]; 6] = [
    [0.0, 1.0, 2.0],
    [0.0, 1.0, 4.5],
    [0.3, 0.8, 3.0],
    [-0.6, 0.4, 6.0],
    [0.5, -0.5, 4.0],
    [0.0, -1.0, 5.0],
];

#[test]
fn a_nested_tree_plays_the_pose_of_its_hand_flattened_leaves() {
    let (mesh, graph) = nested();
    for phase in [0.0, 0.3, 0.7] {
        let mut anim = animator(Motion::Tree("Loco"), None, phase);
        for values in SWEEP {
            set(&mut anim, values);
            // Every clip is linear in x, so the blend is the weighted mean of the
            // leaves' x at the shared phase.
            let want: f32 = flattened(&anim)
                .iter()
                .map(|&(clip, w)| w * CLIPS.iter().find(|c| c.0 == clip).unwrap().1 * phase)
                .sum();
            let pose = sample_pose(&anim, &mesh, Some(&graph)).unwrap().1;
            let got = pose[HIPS].unwrap().translation.x;
            assert!(
                (got - want).abs() < 1e-4,
                "{values:?} at {phase}: {got}, want {want}"
            );
        }
    }
}

#[test]
fn the_shared_phase_runs_at_the_leaves_weighted_cycle_length() {
    let (mesh, graph) = nested();
    let mut anim = animator(Motion::Tree("Loco"), None, 0.0);
    anim.base.loop_clip = true;
    set(&mut anim, [0.3, 0.8, 3.0]);
    let length: f32 = flattened(&anim)
        .iter()
        .map(|&(clip, w)| w * CLIPS.iter().find(|c| c.0 == clip).unwrap().2)
        .sum();
    advance(&mut anim, Some(&mesh), Some(&graph), 0.25 * length);
    assert!(
        (anim.base.time - 0.25).abs() < 1e-5,
        "phase {}",
        anim.base.time
    );
}

#[test]
fn only_the_dominant_leaf_fires_its_events() {
    let (mesh, graph) = nested();
    let fired = |values| {
        let mut anim = animator(Motion::Tree("Loco"), None, 0.0);
        anim.base.loop_clip = true;
        set(&mut anim, values);
        let step = |_| advance(&mut anim, Some(&mesh), Some(&graph), 0.05);
        (0..20).flat_map(step).map(|e| e.name).collect::<Vec<_>>()
    };
    // Forward at walking speed: the nested Walk leaf dominates and steps once.
    assert_eq!(fired([0.0, 1.0, 2.5]), ["walkStep"]);
    // Running forward, or strafing: another leaf dominates, so Walk is silent.
    assert!(fired([0.0, 1.0, 5.5]).is_empty());
    assert!(fired([1.0, 0.1, 2.0]).is_empty());
}
