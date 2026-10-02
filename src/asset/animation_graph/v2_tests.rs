//! The v2 document (#457): blend-tree nodes and layers parse, round-trip and
//! validate; a pre-#457 graph is a graph with no extra layers, unchanged on save.

use super::tests::sample_graph;
use super::*;

const V2: &str = r#"{
  "parameters": { "speed": { "Float": 0.0 }, "velX": { "Float": 0.0 }, "Fire": "Trigger" },
  "nodes": [
    { "name": "Loco", "is_loop": true, "blend_tree": { "Simple1D": { "parameter": "speed",
      "children": [ { "clip": "Idle", "threshold": 0.0 }, { "clip": "Run", "threshold": 6.0 } ] } } }
  ],
  "entry": "Loco",
  "layers": [
    { "name": "Upper", "blending": "Additive", "mask": ["spine_01"],
      "nodes": [ { "name": "Aim", "clip": "RifleAim" } ], "entry": "Aim" }
  ]
}"#;

fn problems(graph: &AnimationGraph) -> String {
    match graph.validate() {
        Err(GraphError::Invalid(problems)) => problems.join("; "),
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn a_v2_document_parses_with_defaults_and_round_trips() {
    let graph: AnimationGraph = serde_json::from_str(V2).unwrap();
    assert_eq!(graph.validate(), Ok(()));
    assert!(matches!(
        graph.base.nodes[0].blend_tree,
        Some(BlendTree::Simple1D { .. })
    ));
    let upper = &graph.layers[0];
    assert_eq!(
        (upper.weight, upper.blending),
        (1.0, LayerBlending::Additive)
    );
    assert_eq!(upper.mask, ["spine_01"]);
    assert_eq!(graph.machine(1).unwrap().entry, "Aim");
    assert!(graph.machine(2).is_none());
    let json = serde_json::to_string_pretty(&graph).unwrap();
    assert_eq!(
        serde_json::from_str::<AnimationGraph>(&json).unwrap(),
        graph
    );
}

#[test]
fn a_single_layer_graph_saves_exactly_as_before_layers() {
    let json = serde_json::to_string_pretty(&sample_graph()).unwrap();
    assert!(!json.contains("layers") && !json.contains("blend_tree") && !json.contains("base"));
    let back: AnimationGraph = serde_json::from_str(&json).unwrap();
    assert!(back.layers.is_empty());
}

#[test]
fn malformed_blend_trees_are_rejected() {
    let mut graph: AnimationGraph = serde_json::from_str(V2).unwrap();
    graph
        .parameters
        .insert("speed".into(), ParameterDeclaration::Int(0));
    assert!(problems(&graph).contains("reads 'speed', which is not a declared Float"));

    let mut graph: AnimationGraph = serde_json::from_str(V2).unwrap();
    if let Some(BlendTree::Simple1D { children, .. }) = &mut graph.base.nodes[0].blend_tree {
        children[1].threshold = 0.0;
        children[0].clip = String::new();
    }
    let report = problems(&graph);
    assert!(report.contains("two children at the same position"));
    assert!(report.contains("a child with an empty clip name"));

    graph.base.nodes[0].blend_tree = Some(BlendTree::FreeformCartesian2D {
        parameter_x: "velX".into(),
        parameter_y: "velZ".into(),
        children: Vec::new(),
    });
    graph.base.nodes[0].clip = "Idle".into();
    let report = problems(&graph);
    assert!(report.contains("both a clip and a blend tree"));
}

#[test]
fn layer_problems_are_reported_under_the_layer() {
    let mut graph: AnimationGraph = serde_json::from_str(V2).unwrap();
    let mut second = graph.layers[0].clone();
    second.weight = 1.5;
    second.mask.push(String::new());
    second.machine.entry = "Nope".into();
    graph.layers.push(second);
    let report = problems(&graph);
    assert!(report.contains("duplicate layer name 'Upper'"));
    assert!(report.contains("layer #2 'Upper' has a weight outside [0, 1]"));
    assert!(report.contains("layer #2 'Upper' masks an empty bone name"));
    assert!(report.contains("layer #2 'Upper': entry node 'Nope' does not exist"));
}
