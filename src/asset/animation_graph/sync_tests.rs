//! Sync groups and markers on the graph asset (#1896): they parse onto clip
//! nodes and blend-tree children, load sorted by time, round-trip, stay out of
//! a graph that doesn't use them, and malformed ones are rejected.

use super::*;

const SYNCED: &str = r#"{
  "parameters": { "speed": { "Float": 0.0 } },
  "nodes": [
    { "name": "Walk", "clip": "Walk", "is_loop": true, "sync_group": "Locomotion",
      "sync_markers": [ { "time": 0.54, "name": "FootR" }, { "time": 0.02, "name": "FootL" } ] },
    { "name": "Loco", "is_loop": true, "sync_group": "Locomotion",
      "blend_tree": { "Simple1D": { "parameter": "speed", "children": [
        { "clip": "Walk", "threshold": 0.0, "sync_markers": [ { "time": 0.0, "name": "FootL" } ] },
        { "clip": "Run", "threshold": 1.0 } ] } } }
  ],
  "entry": "Walk"
}"#;

fn load_text(text: &str) -> Result<AnimationGraph, GraphError> {
    let name = format!(
        "rusty-sync-{}-{:x}.animgraph",
        std::process::id(),
        text.len()
    );
    let path = std::env::temp_dir().join(name);
    std::fs::write(&path, text).unwrap();
    let loaded = load(&path);
    let _ = std::fs::remove_file(&path);
    loaded
}

fn problems(graph: &AnimationGraph) -> String {
    match graph.validate() {
        Err(GraphError::Invalid(problems)) => problems.join("; "),
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn markers_load_sorted_by_time_and_round_trip() {
    let graph = load_text(SYNCED).unwrap();
    let walk = &graph.base.nodes[0];
    assert_eq!(walk.sync_group.as_deref(), Some("Locomotion"));
    let names: Vec<_> = walk.sync_markers.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, ["FootL", "FootR"]);
    let json = serde_json::to_string(&graph).unwrap();
    assert_eq!(
        serde_json::from_str::<AnimationGraph>(&json).unwrap(),
        graph
    );
}

#[test]
fn a_graph_without_sync_writes_no_sync_keys() {
    let mut graph: AnimationGraph = serde_json::from_str(SYNCED).unwrap();
    for node in &mut graph.base.nodes {
        node.sync_group = None;
        node.sync_markers.clear();
    }
    let tree = graph.base.nodes[1].blend_tree.as_mut().unwrap();
    let BlendTree::Simple1D { children, .. } = tree else {
        unreachable!()
    };
    children[0].motion.sync_markers.clear();
    assert!(!serde_json::to_string(&graph).unwrap().contains("sync"));
}

#[test]
fn a_tree_marked_on_some_leaves_only_warns() {
    let graph = load_text(SYNCED).unwrap();
    let tree = graph.base.nodes[1].blend_tree.as_ref().unwrap();
    assert!(!tree.marker_synced());
    let warnings = graph.warnings().join("; ");
    assert!(warnings.contains("node 'Loco' blend tree has sync markers on some leaves only"));
    let mut whole = graph.clone();
    whole.base.nodes.truncate(1);
    assert!(whole.warnings().is_empty());
}

#[test]
fn malformed_markers_are_rejected_by_path() {
    let mut graph: AnimationGraph = serde_json::from_str(SYNCED).unwrap();
    let marker = |time: f32, name: &str| SyncMarker {
        time,
        name: name.to_string(),
    };
    let walk = &mut graph.base.nodes[0];
    walk.sync_markers = vec![
        marker(0.1, "FootL"),
        marker(f32::NAN, "X"),
        marker(0.1, "FootL"),
    ];
    walk.sync_group = Some(String::new());
    graph.base.nodes[1].sync_markers = vec![marker(0.0, "Bad")];
    let found = problems(&graph);
    for expected in [
        "node 'Walk' has two sync markers 'FootL' at 0.1",
        "node 'Walk' sync marker 'X' has a negative or non-finite time",
        "node 'Walk' has an empty sync group name",
        "node 'Loco' is a blend tree with sync markers of its own",
    ] {
        assert!(found.contains(expected), "missing {expected:?} in {found}");
    }
}

#[test]
fn a_duplicate_marker_fails_the_load() {
    let text = SYNCED.replace(
        r#""time": 0.54, "name": "FootR""#,
        r#""time": 0.02, "name": "FootL""#,
    );
    let err = load_text(&text).unwrap_err().to_string();
    assert!(
        err.contains("node 'Walk' has two sync markers 'FootL'"),
        "{err}"
    );
}
