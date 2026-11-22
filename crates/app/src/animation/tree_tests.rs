use super::test_rig::*;
use super::*;
use rusty_asset::animation_graph::{BlendChild1D, BlendTree, ChildMotion, GraphNode};
use rusty_components::Motion;

pub fn tree_node(name: &str, children: &[(&str, f32)]) -> GraphNode {
    let children = children
        .iter()
        .map(|&(clip, threshold)| BlendChild1D {
            motion: ChildMotion::clip(clip),
            threshold,
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
    for (speed, x) in [(0.0, 1.0), (2.0, 1.0), (4.0, 2.0), (6.0, 3.0)] {
        anim.set_float("speed", speed);
        let got = hips_x(&anim, &mesh, &graph);
        assert!((got - x).abs() < 1e-5, "speed {speed}: x {got}, want {x}");
    }
    let pose = sample_pose(&anim, &mesh, Some(&graph)).unwrap().1;
    assert!(pose[SPINE].is_none() && pose[LEG].is_none());
}

#[test]
fn the_tree_playhead_runs_at_its_weighted_cycle_length_and_wraps() {
    let (mesh, graph) = locomotion();
    let mut anim = animator(Motion::Tree("Loco"), None, 0.0);
    anim.base.loop_clip = true;
    anim.set_float("speed", 4.0);
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
    assert!((hips_x(&anim, &mesh, &graph) - 1.0).abs() < 1e-5);
    anim.play("Walk".to_string());
    assert_eq!(anim.base.motion(), Motion::Clip("Walk"));
    assert_eq!(anim.base.current_tree, None);
}
