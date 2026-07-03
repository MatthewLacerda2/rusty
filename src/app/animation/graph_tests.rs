//! Unit tests for the graph runtime evaluator (#316): binding seeds defaults and
//! enters the entry node, conditions gate transitions in authored priority order,
//! a fired trigger is consumed exactly once, condition-less edges never auto-fire,
//! and a fixed-dt replay is byte-identical.

use std::collections::BTreeMap;

use super::*;
use crate::asset::animation_graph::{GraphNode, ParameterDeclaration};

fn node(name: &str, clip: &str, is_loop: bool, speed: Option<f32>) -> GraphNode {
    GraphNode {
        name: name.to_string(),
        clip: clip.to_string(),
        is_loop,
        speed,
    }
}

fn edge(from: &str, to: &str, conditions: Vec<Condition>, duration: f32) -> GraphEdge {
    GraphEdge {
        from: from.to_string(),
        to: to.to_string(),
        conditions,
        transition_duration: duration,
    }
}

fn float_cond(parameter: &str, op: NumericOp, value: f32) -> Condition {
    Condition::Float {
        parameter: parameter.to_string(),
        op,
        value,
    }
}

/// Idle ⇄ Run on the `speed` float, Idle → Jump on the `Jump` trigger.
fn locomotion() -> AnimationGraph {
    AnimationGraph {
        parameters: BTreeMap::from([
            ("speed".to_string(), ParameterDeclaration::Float(0.0)),
            ("armed".to_string(), ParameterDeclaration::Bool(true)),
            ("Jump".to_string(), ParameterDeclaration::Trigger),
        ]),
        nodes: vec![
            node("Idle", "IdleClip", true, None),
            node("Run", "RunClip", true, Some(2.0)),
            node("Jump", "JumpClip", false, None),
        ],
        edges: vec![
            edge(
                "Idle",
                "Run",
                vec![float_cond("speed", NumericOp::Greater, 1.0)],
                0.25,
            ),
            edge(
                "Idle",
                "Jump",
                vec![Condition::Trigger {
                    parameter: "Jump".to_string(),
                }],
                0.0,
            ),
            edge(
                "Run",
                "Idle",
                vec![float_cond("speed", NumericOp::LessEqual, 1.0)],
                0.1,
            ),
        ],
        entry: "Idle".to_string(),
    }
}

fn graph_animator() -> AnimatorComponent {
    AnimatorComponent {
        graph: Some("locomotion.animgraph".to_string()),
        ..Default::default()
    }
}

#[test]
fn first_step_binds_seeds_defaults_and_enters_entry() {
    let graph = locomotion();
    let mut anim = graph_animator();
    step_graph(&mut anim, &graph);
    assert_eq!(anim.current_node.as_deref(), Some("Idle"));
    assert_eq!(anim.current_clip, "IdleClip");
    assert!(anim.is_playing && anim.loop_clip);
    assert!(!anim.is_crossfading(), "entry is a hard cut");
    // Declared defaults seeded; the trigger starts clear.
    assert_eq!(anim.get_float("speed"), Some(0.0));
    assert_eq!(anim.get_bool("armed"), Some(true));
    assert!(!anim.trigger_is_set("Jump"));
}

#[test]
fn binding_never_overwrites_a_value_a_script_already_wrote() {
    let graph = locomotion();
    let mut anim = graph_animator();
    anim.set_float("speed", 4.0);
    step_graph(&mut anim, &graph);
    assert_eq!(anim.get_float("speed"), Some(4.0), "script value wins");
}

#[test]
fn satisfied_edge_crossfades_and_adopts_the_target_node() {
    let graph = locomotion();
    let mut anim = graph_animator();
    step_graph(&mut anim, &graph); // bind → Idle
    anim.set_float("speed", 2.0);
    step_graph(&mut anim, &graph);
    assert_eq!(anim.current_node.as_deref(), Some("Run"));
    assert_eq!(anim.current_clip, "RunClip");
    assert_eq!(anim.previous_clip.as_deref(), Some("IdleClip"));
    assert_eq!(anim.crossfade_duration, 0.25, "the edge's blend length");
    assert_eq!(anim.node_speed, 2.0, "per-node speed adopted");
    // Back below the threshold: the Run → Idle edge brings it home.
    anim.set_float("speed", 0.5);
    step_graph(&mut anim, &graph);
    assert_eq!(anim.current_node.as_deref(), Some("Idle"));
    assert_eq!(anim.node_speed, 1.0, "speed override dropped with the node");
}

#[test]
fn unsatisfied_edges_keep_the_current_node_playing() {
    let graph = locomotion();
    let mut anim = graph_animator();
    step_graph(&mut anim, &graph);
    step_graph(&mut anim, &graph); // speed = 0.0, no trigger: nothing fires
    assert_eq!(anim.current_node.as_deref(), Some("Idle"));
    assert!(!anim.is_crossfading());
}

#[test]
fn fired_trigger_is_consumed_exactly_once() {
    let graph = locomotion();
    let mut anim = graph_animator();
    step_graph(&mut anim, &graph); // bind → Idle
    anim.set_trigger("Jump");
    step_graph(&mut anim, &graph);
    assert_eq!(anim.current_node.as_deref(), Some("Jump"));
    assert!(!anim.loop_clip, "Jump's is_loop = false adopted");
    assert!(!anim.trigger_is_set("Jump"), "read-and-clear on fire");
    step_graph(&mut anim, &graph);
    assert_eq!(
        anim.current_node.as_deref(),
        Some("Jump"),
        "the cleared trigger cannot fire twice"
    );
}

#[test]
fn first_authored_edge_wins_when_several_are_satisfiable() {
    let mut graph = locomotion();
    // A second satisfiable Idle edge, authored after the Idle → Run one.
    graph.edges.push(edge(
        "Idle",
        "Jump",
        vec![float_cond("speed", NumericOp::Greater, 0.5)],
        0.0,
    ));
    let mut anim = graph_animator();
    step_graph(&mut anim, &graph);
    anim.set_float("speed", 2.0); // satisfies both Idle edges
    step_graph(&mut anim, &graph);
    assert_eq!(anim.current_node.as_deref(), Some("Run"), "priority order");
}

#[test]
fn condition_less_edges_never_fire_from_data_alone() {
    let mut graph = locomotion();
    graph.edges = vec![edge("Idle", "Run", Vec::new(), 0.0)];
    let mut anim = graph_animator();
    step_graph(&mut anim, &graph);
    step_graph(&mut anim, &graph);
    assert_eq!(anim.current_node.as_deref(), Some("Idle"));
}

#[test]
fn a_stale_active_node_rebinds_to_the_entry() {
    let graph = locomotion();
    let mut anim = graph_animator();
    anim.current_node = Some("Ghost".to_string()); // e.g. the asset was re-authored
    step_graph(&mut anim, &graph);
    assert_eq!(anim.current_node.as_deref(), Some("Idle"));
}

#[test]
fn every_numeric_operator_compares_correctly() {
    use NumericOp::*;
    assert!(compare(1, Less, 2) && !compare(2, Less, 2));
    assert!(compare(2, LessEqual, 2) && !compare(3, LessEqual, 2));
    assert!(compare(3, Greater, 2) && !compare(2, Greater, 2));
    assert!(compare(2, GreaterEqual, 2) && !compare(1, GreaterEqual, 2));
    assert!(compare(2, Equal, 2) && !compare(1, Equal, 2));
    assert!(compare(1, NotEqual, 2) && !compare(2, NotEqual, 2));
}

#[test]
fn bool_condition_requires_the_matching_value_and_type() {
    let mut anim = AnimatorComponent::default();
    let is_false = Condition::Bool {
        parameter: "armed".to_string(),
        value: false,
    };
    assert!(!condition_holds(&anim, &is_false), "absent never satisfies");
    anim.set_bool("armed", false);
    assert!(condition_holds(&anim, &is_false));
    anim.parameters.clear();
    anim.set_int("armed", 0); // wrong type never satisfies
    assert!(!condition_holds(&anim, &is_false));
}

/// Determinism: the same (graph, parameter writes, fixed dt) sequence yields a
/// byte-identical animator — the property the headless replay depends on.
#[test]
fn replaying_the_same_inputs_is_byte_identical() {
    let run = || {
        let graph = locomotion();
        let mut anim = graph_animator();
        for i in 0..120u32 {
            if i == 30 {
                anim.set_float("speed", 3.0);
            }
            if i == 60 {
                anim.set_float("speed", 0.25);
            }
            if i == 90 {
                anim.set_trigger("Jump");
            }
            step_graph(&mut anim, &graph);
            anim.advance(1.0 / 60.0, 1.5);
        }
        serde_json::to_string(&anim).unwrap()
    };
    assert_eq!(run(), run());
}
