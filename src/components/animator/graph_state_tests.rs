//! Unit tests for the graph-state writes (#316): entering a node adopts its clip,
//! loop flag and speed; binding seeds declared defaults without clobbering script
//! writes; the per-node speed stacks into `advance`; and the new graph fields
//! round-trip through serde with the entity.

use std::collections::BTreeMap;

use crate::asset::animation_graph::{AnimationGraph, GraphNode, ParameterDeclaration};

use super::{AnimatorComponent, AnimatorParameter};

fn run_node() -> GraphNode {
    GraphNode {
        name: "Run".to_string(),
        clip: "RunClip".to_string(),
        is_loop: true,
        speed: Some(2.0),
    }
}

fn one_node_graph() -> AnimationGraph {
    AnimationGraph {
        parameters: BTreeMap::from([
            ("speed".to_string(), ParameterDeclaration::Float(1.5)),
            ("Jump".to_string(), ParameterDeclaration::Trigger),
        ]),
        nodes: vec![run_node()],
        edges: Vec::new(),
        entry: "Run".to_string(),
    }
}

#[test]
fn enter_node_with_a_blend_crossfades_into_the_node() {
    let mut anim = AnimatorComponent {
        current_clip: "IdleClip".to_string(),
        is_playing: true,
        ..Default::default()
    };
    anim.enter_node(&run_node(), 0.3);
    assert_eq!(anim.current_node.as_deref(), Some("Run"));
    assert_eq!(anim.current_clip, "RunClip");
    assert_eq!(anim.previous_clip.as_deref(), Some("IdleClip"));
    assert_eq!(anim.crossfade_duration, 0.3);
    assert!(anim.loop_clip && anim.is_playing);
    assert_eq!(anim.node_speed, 2.0);
}

#[test]
fn enter_node_with_zero_blend_is_a_hard_cut() {
    let mut anim = AnimatorComponent::default();
    anim.enter_node(&run_node(), 0.0);
    assert!(!anim.is_crossfading());
    assert_eq!(anim.current_clip, "RunClip");
    assert_eq!(anim.time, 0.0);
}

#[test]
fn bind_graph_seeds_defaults_and_enters_the_entry_node() {
    let mut anim = AnimatorComponent::default();
    anim.set_float("speed", 9.0); // a script wrote this before the bind
    anim.bind_graph(&one_node_graph());
    assert_eq!(anim.current_node.as_deref(), Some("Run"));
    assert_eq!(anim.get_float("speed"), Some(9.0), "script value wins");
    assert!(!anim.trigger_is_set("Jump"), "a trigger starts clear");
    assert_eq!(
        anim.parameters.get("Jump"),
        Some(&AnimatorParameter::Trigger(false))
    );
}

#[test]
fn advance_stacks_component_and_node_speed() {
    let mut anim = AnimatorComponent {
        is_playing: true,
        speed: 2.0,
        node_speed: 0.5,
        ..Default::default()
    };
    anim.advance(1.0, 0.0);
    assert_eq!(anim.time, 1.0, "dt * speed * node_speed = 1.0 * 2.0 * 0.5");
}

#[test]
fn graph_fields_round_trip_through_serde() {
    let anim = AnimatorComponent {
        graph: Some("guard.animgraph".to_string()),
        graph_enabled: false,
        current_node: Some("Run".to_string()),
        node_speed: 2.0,
        ..Default::default()
    };
    let json = serde_json::to_string(&anim).unwrap();
    let back: AnimatorComponent = serde_json::from_str(&json).unwrap();
    assert_eq!(back.graph.as_deref(), Some("guard.animgraph"));
    assert!(!back.graph_enabled);
    assert_eq!(back.current_node.as_deref(), Some("Run"));
    assert_eq!(back.node_speed, 2.0);
}

#[test]
fn default_graph_fields_stay_out_of_the_serialized_form() {
    // The defaults (enabled, no node, 1× speed) are skipped, so pre-#316 scenes
    // and post-#316 scenes serialize identically for a graph-less animator...
    let json = serde_json::to_string(&AnimatorComponent::default()).unwrap();
    for key in ["graph", "current_node", "node_speed", "graph_enabled"] {
        assert!(!json.contains(key), "default `{key}` must not serialize");
    }
    // ...and a document missing them deserializes to the same defaults.
    let back: AnimatorComponent = serde_json::from_str(&json).unwrap();
    assert!(back.graph_enabled, "graph evaluation defaults to enabled");
    assert_eq!(back.node_speed, 1.0);
    assert_eq!(back.current_node, None);
}
