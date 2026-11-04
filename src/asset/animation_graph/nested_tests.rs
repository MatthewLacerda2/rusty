//! Nested blend trees on the graph asset (#1746): a child holding a tree parses
//! and round-trips, and every level is validated, each problem naming its path.

use super::*;

const NESTED: &str = r#"{
  "parameters": { "velX": { "Float": 0.0 }, "velZ": { "Float": 0.0 }, "speed": { "Float": 0.0 } },
  "nodes": [ { "name": "Loco", "is_loop": true, "blend_tree": { "FreeformDirectional2D": {
    "parameter_x": "velX", "parameter_y": "velZ",
    "children": [
      { "position": [0, 1], "blend_tree": { "Simple1D": { "parameter": "speed",
          "children": [ { "clip": "WalkFwd", "threshold": 2 },
                        { "clip": "RunFwd", "threshold": 6 } ] } } },
      { "position": [0, -1], "clip": "WalkBack" } ] } } } ],
  "entry": "Loco"
}"#;

fn parsed() -> AnimationGraph {
    serde_json::from_str(NESTED).unwrap()
}

fn problems(graph: &AnimationGraph) -> String {
    match graph.validate() {
        Err(GraphError::Invalid(problems)) => problems.join("; "),
        other => panic!("expected Invalid, got {other:?}"),
    }
}

/// The nested 1D tree of the parsed graph.
fn inner(graph: &mut AnimationGraph) -> &mut BlendTree {
    let Some(BlendTree::FreeformDirectional2D { children, .. }) =
        &mut graph.base.nodes[0].blend_tree
    else {
        panic!("a 2D root");
    };
    children[0].motion.blend_tree.as_deref_mut().unwrap()
}

#[test]
fn a_nested_tree_parses_and_round_trips() {
    let mut graph = parsed();
    assert_eq!(graph.validate(), Ok(()));
    assert!(matches!(inner(&mut graph), BlendTree::Simple1D { .. }));
    let json = serde_json::to_string(&graph).unwrap();
    assert_eq!(
        serde_json::from_str::<AnimationGraph>(&json).unwrap(),
        graph
    );
    assert!(
        !json.contains(r#""clip":"""#),
        "an empty clip is not written"
    );
}

#[test]
fn every_level_is_validated_under_its_path() {
    let mut graph = parsed();
    graph.parameters.remove("speed");
    if let BlendTree::Simple1D { children, .. } = inner(&mut graph) {
        children[1].threshold = 2.0;
        children[0].motion.clip = String::new();
        children[1].motion.blend_tree = Some(Box::new(BlendTree::Simple1D {
            parameter: "velX".into(),
            children: Vec::new(),
        }));
    }
    let found = problems(&graph);
    for expected in [
        "blend tree 'Loco/children[0]' reads 'speed', which is not a declared Float",
        "blend tree 'Loco/children[0]' has two children at the same position",
        "child 'Loco/children[0]/children[0]' has neither a clip nor a blend tree",
        "child 'Loco/children[0]/children[1]' has both a clip and a blend tree",
    ] {
        assert!(found.contains(expected), "missing '{expected}' in: {found}");
    }
}

#[test]
fn a_child_holding_a_tree_carries_no_events_of_its_own() {
    let mut graph = parsed();
    if let Some(BlendTree::FreeformDirectional2D { children, .. }) =
        &mut graph.base.nodes[0].blend_tree
    {
        children[0].motion.events.push(AnimationEvent {
            time: 0.1,
            name: "Step".into(),
        });
    }
    assert!(problems(&graph).contains("blend tree 'Loco/children[0]' has events of its own"));
}

#[test]
fn trees_nest_at_most_the_depth_cap() {
    let chain = |depth: usize| {
        let leaf = BlendChild1D {
            motion: ChildMotion::clip("Walk"),
            threshold: 0.0,
        };
        let mut tree = BlendTree::Simple1D {
            parameter: "speed".into(),
            children: vec![leaf],
        };
        for _ in 1..depth {
            let motion = ChildMotion::tree(tree);
            let child = BlendChild1D {
                motion,
                threshold: 0.0,
            };
            tree = BlendTree::Simple1D {
                parameter: "speed".into(),
                children: vec![child],
            };
        }
        let mut graph = parsed();
        graph.base.nodes[0].blend_tree = Some(tree);
        graph
    };
    assert_eq!(chain(MAX_TREE_DEPTH).validate(), Ok(()));
    assert!(problems(&chain(MAX_TREE_DEPTH + 1)).contains("nests trees deeper than 8"));
}

#[test]
fn one_load_reports_a_tree_under_a_child_that_also_has_a_clip() {
    let mut graph = parsed();
    graph.parameters.remove("speed");
    if let Some(BlendTree::FreeformDirectional2D { children, .. }) =
        &mut graph.base.nodes[0].blend_tree
    {
        children[0].motion.clip = "Fwd".into();
        let event = |time: f32| AnimationEvent {
            time,
            name: "".into(),
        };
        children[0].motion.events.push(event(-1.0));
    }
    let found = problems(&graph);
    for expected in [
        "child 'Loco/children[0]' has both a clip and a blend tree",
        "blend tree 'Loco/children[0]' reads 'speed'",
        "blend tree 'Loco/children[0]' has events of its own",
        "child 'Fwd' has an event with an empty name",
        "event '' has a negative or non-finite time",
    ] {
        assert!(found.contains(expected), "missing '{expected}' in: {found}");
    }
}
