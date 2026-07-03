//! Serde + io tests for the `AnimationGraph` asset (#315): a representative graph
//! round-trips losslessly, the JSON is deterministic and name-sorted, absent
//! optional fields default, and load/save enforce validation at the file boundary.

use std::collections::BTreeMap;

use super::*;

/// A representative graph: all four parameter types declared, three nodes
/// (loop/speed variations), and edges exercising every condition kind plus a
/// numeric-operator spread.
pub(super) fn sample_graph() -> AnimationGraph {
    let parameters = BTreeMap::from([
        ("isGrounded".into(), ParameterDeclaration::Bool(true)),
        ("speed".into(), ParameterDeclaration::Float(0.0)),
        ("weapon".into(), ParameterDeclaration::Int(0)),
        ("Jump".into(), ParameterDeclaration::Trigger),
    ]);
    let node = |name: &str, clip: &str, is_loop, speed| GraphNode {
        name: name.into(),
        clip: clip.into(),
        is_loop,
        speed,
    };
    AnimationGraph {
        parameters,
        nodes: vec![
            node("idle", "Idle", true, None),
            node("run", "Run", true, Some(1.5)),
            node("jump", "Jump_Start", false, None),
        ],
        edges: sample_edges(),
        entry: "idle".into(),
    }
}

/// The sample graph's edges, exercising every condition kind.
fn sample_edges() -> Vec<GraphEdge> {
    let cond_speed = |op, value| Condition::Float {
        parameter: "speed".into(),
        op,
        value,
    };
    vec![
        GraphEdge {
            from: "idle".into(),
            to: "run".into(),
            conditions: vec![
                cond_speed(NumericOp::Greater, 0.2),
                Condition::Bool {
                    parameter: "isGrounded".into(),
                    value: true,
                },
            ],
            transition_duration: 0.25,
        },
        GraphEdge {
            from: "run".into(),
            to: "idle".into(),
            conditions: vec![
                cond_speed(NumericOp::LessEqual, 0.2),
                Condition::Int {
                    parameter: "weapon".into(),
                    op: NumericOp::NotEqual,
                    value: 2,
                },
            ],
            transition_duration: 0.25,
        },
        GraphEdge {
            from: "idle".into(),
            to: "jump".into(),
            conditions: vec![Condition::Trigger {
                parameter: "Jump".into(),
            }],
            transition_duration: 0.0,
        },
    ]
}

#[test]
fn round_trips_through_json_losslessly_and_deterministically() {
    let graph = sample_graph();
    let json = serde_json::to_string_pretty(&graph).unwrap();
    let back: AnimationGraph = serde_json::from_str(&json).unwrap();
    assert_eq!(back, graph);
    // Same input, same bytes — the document is deterministic (BTreeMap keeps the
    // parameter keys name-sorted, Vecs keep authored order).
    assert_eq!(serde_json::to_string_pretty(&back).unwrap(), json);
    let jump = json.find("\"Jump\"").unwrap();
    let is_grounded = json.find("\"isGrounded\"").unwrap();
    assert!(jump < is_grounded, "parameter keys serialize name-sorted");
}

#[test]
fn absent_optional_fields_default() {
    // A minimal hand-written document: no parameters, no edges, and a node with
    // neither `is_loop` nor `speed`.
    let json = r#"{ "nodes": [ { "name": "idle", "clip": "Idle" } ], "entry": "idle" }"#;
    let graph: AnimationGraph = serde_json::from_str(json).unwrap();
    assert!(graph.parameters.is_empty() && graph.edges.is_empty());
    assert!(!graph.nodes[0].is_loop);
    assert_eq!(graph.nodes[0].speed, None);
    assert!(graph.validate().is_ok());
}

#[test]
fn save_then_load_round_trips_and_load_rejects_bad_files() {
    let dir = std::env::temp_dir().join("rusty_animgraph_io");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("guard.{GRAPH_EXTENSION}"));
    let graph = sample_graph();
    save(&path, &graph).unwrap();
    assert_eq!(load(&path).unwrap(), graph);

    assert!(matches!(
        load(&dir.join("missing.animgraph")),
        Err(GraphError::Io(_))
    ));
    let garbage = dir.join("garbage.animgraph");
    std::fs::write(&garbage, "not a graph").unwrap();
    assert!(matches!(load(&garbage), Err(GraphError::Parse(_))));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn io_validates_both_directions() {
    let dir = std::env::temp_dir().join("rusty_animgraph_invalid");
    std::fs::create_dir_all(&dir).unwrap();
    let mut graph = sample_graph();
    graph.entry = "nope".into();
    // An invalid graph can't reach disk from code…
    let path = dir.join("bad.animgraph");
    assert!(matches!(save(&path, &graph), Err(GraphError::Invalid(_))));
    // …and a hand-written invalid document can't load either.
    std::fs::write(&path, serde_json::to_string(&graph).unwrap()).unwrap();
    assert!(matches!(load(&path), Err(GraphError::Invalid(_))));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn graph_paths_are_recognised_by_extension() {
    assert!(is_graph_path("project/anims/guard.animgraph"));
    assert!(is_graph_path("GUARD.ANIMGRAPH"));
    assert!(!is_graph_path("project/models/crates.glb"));
    assert!(!is_graph_path("animgraph"));
}
