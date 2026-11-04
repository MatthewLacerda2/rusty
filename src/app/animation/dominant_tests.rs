//! Which leaf of a nested blend tree fires its events (#1746): the heaviest
//! child at each level, so a direction split into walk and run still beats a
//! lighter direction, and a leaf whose clip the mesh lacks never silences them.

use super::blend_tree::nested_tests::directional;
use super::blend_tree::{child_weights, dominant_leaf};
use super::nested_pose_tests::{nested, set, walk_run, CLIPS};
use super::test_rig::*;
use super::*;
use crate::asset::animation_graph::{AnimationEvent, BlendTree, ChildMotion};
use crate::components::Motion;

/// Forward mostly, a little right, halfway between walking and running.
const FORWARD_RIGHT: [f32; 3] = [0.4, 0.6, 4.0];

/// `graph`'s Right child, given a `rightStep` marker at its midpoint.
fn with_right_steps(graph: &mut AnimationGraph) {
    let Some(BlendTree::FreeformDirectional2D { children, .. }) =
        &mut graph.base.nodes[0].blend_tree
    else {
        panic!("a 2D root");
    };
    children[3].motion.events = vec![AnimationEvent {
        time: 0.45,
        name: "rightStep".into(),
    }];
}

fn fired(mesh: &MeshComponent, graph: &AnimationGraph, values: [f32; 3]) -> Vec<String> {
    let mut anim = animator(Motion::Tree("Loco"), None, 0.0);
    anim.base.loop_clip = true;
    set(&mut anim, values);
    let step = |_| advance(&mut anim, Some(mesh), Some(graph), 0.05);
    (0..40).flat_map(step).map(|e| e.name).collect()
}

#[test]
fn the_heaviest_direction_fires_even_when_its_weight_is_split() {
    let (mesh, mut graph) = nested();
    with_right_steps(&mut graph);
    let mut anim = animator(Motion::Tree("Loco"), None, 0.0);
    set(&mut anim, FORWARD_RIGHT);
    let top: Vec<f32> =
        child_weights(&directional(ChildMotion::clip("Walk")), &anim.parameters).collect();
    let inner: Vec<f32> = child_weights(&walk_run(), &anim.parameters).collect();
    // The case the rule exists for: forward outweighs right, yet each of its
    // leaves alone weighs less than right.
    assert!(top[0] > top[3], "{top:?}");
    assert!(top[0] * inner[0] < top[3] && top[0] * inner[1] < top[3]);
    let names = fired(&mesh, &graph, FORWARD_RIGHT);
    assert!(names.iter().all(|n| n == "walkStep"), "{names:?}");
    assert!(!names.is_empty());
}

#[test]
fn a_leaf_the_mesh_lacks_is_passed_over_for_events() {
    let (_, graph) = nested();
    let clips = CLIPS.iter().filter(|c| c.0 != "Run");
    let mesh = mesh(
        clips
            .map(|&(n, x, d)| slide(n, &[HIPS], 0.0, x, d))
            .collect(),
    );
    // Running forward: Run would dominate, but it doesn't resolve, so Walk steps.
    assert!(fired(&mesh, &graph, [0.0, 1.0, 5.5]).contains(&"walkStep".to_string()));
}

#[test]
fn the_walk_goes_down_one_child_per_level() {
    let tree = directional(ChildMotion::tree(walk_run()));
    let mut anim = animator(Motion::Tree("Loco"), None, 0.0);
    set(&mut anim, FORWARD_RIGHT);
    let all = |_: &str| true;
    let leaf = dominant_leaf(&tree, &anim.parameters, &all).unwrap();
    // Walk and Run tie inside forward; the first wins.
    assert_eq!(leaf.clip, "Walk");
    let no_forward = |c: &str| c != "Walk" && c != "Run";
    let leaf = dominant_leaf(&tree, &anim.parameters, &no_forward).unwrap();
    assert_eq!(leaf.clip, "Right");
    assert!(dominant_leaf(&tree, &anim.parameters, &|_: &str| false).is_none());
}
