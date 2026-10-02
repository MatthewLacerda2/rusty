//! Animation events on the graph asset (#459): they parse onto clip nodes and
//! blend-tree children, round-trip, and malformed ones are rejected.

use super::*;

const EVENTS: &str = r#"{
  "parameters": { "speed": { "Float": 0.0 } },
  "nodes": [
    { "name": "Reload", "clip": "Reload", "events": [ { "time": 1.2, "name": "MagIn" } ] },
    { "name": "Loco", "is_loop": true, "blend_tree": { "Simple1D": { "parameter": "speed",
      "children": [ { "clip": "Walk", "threshold": 2.0, "events": [ { "time": 0.4, "name": "Step" } ] },
                    { "clip": "Run", "threshold": 6.0 } ] } } }
  ],
  "entry": "Loco"
}"#;

fn parsed() -> AnimationGraph {
    serde_json::from_str(EVENTS).unwrap()
}

fn problems(graph: &AnimationGraph) -> String {
    match graph.validate() {
        Err(GraphError::Invalid(problems)) => problems.join("; "),
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn events_parse_onto_nodes_and_children_and_round_trip() {
    let graph = parsed();
    assert_eq!(graph.validate(), Ok(()));
    let mag_in = &graph.base.nodes[0].events[0];
    assert_eq!((mag_in.time, mag_in.name.as_str()), (1.2, "MagIn"));
    let tree = graph.base.nodes[1].blend_tree.as_ref().unwrap();
    let events = tree.child_events();
    assert_eq!((events[0][0].name.as_str(), events[1].len()), ("Step", 0));
    let json = serde_json::to_string(&graph).unwrap();
    assert_eq!(
        serde_json::from_str::<AnimationGraph>(&json).unwrap(),
        graph
    );
    assert_eq!(
        json.matches("\"events\"").count(),
        2,
        "empty lists are not written"
    );
}

#[test]
fn malformed_events_are_rejected() {
    let mut graph = parsed();
    graph.base.nodes[0].events.push(AnimationEvent {
        time: -1.0,
        name: String::new(),
    });
    graph.base.nodes[1].events = graph.base.nodes[0].events.clone();
    if let Some(BlendTree::Simple1D { children, .. }) = &mut graph.base.nodes[1].blend_tree {
        children[1].events.push(AnimationEvent {
            time: f32::NAN,
            name: "Bad".to_string(),
        });
    }
    let found = problems(&graph);
    for expected in [
        "node 'Reload' has an event with an empty name",
        "node 'Reload' event '' has a negative or non-finite time",
        "node 'Loco' is a blend tree with events of its own",
        "node 'Loco' blend tree child 'Run' event 'Bad' has a negative or non-finite time",
    ] {
        assert!(found.contains(expected), "missing '{expected}' in: {found}");
    }
}
