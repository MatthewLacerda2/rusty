//! A node's `tag` (#1901): it parses, validates, survives a save and load, and
//! a node without one saves exactly as before (no `tag` key).

use super::*;

const TAGGED: &str = r#"{
  "nodes": [ { "name": "Aim", "clip": "Aim", "tag": "attack" }, { "name": "Idle", "clip": "Idle" } ],
  "entry": "Idle"
}"#;

#[test]
fn a_tag_round_trips_through_save_and_load() {
    let graph: AnimationGraph = serde_json::from_str(TAGGED).unwrap();
    assert_eq!(graph.validate(), Ok(()));
    assert_eq!(graph.node("Aim").unwrap().tag, "attack");
    assert_eq!(graph.node("Idle").unwrap().tag, "", "no tag reads as empty");
    let path = rusty_test_support::temp::dir().join("rusty_1901_tag.animgraph");
    save(&path, &graph).unwrap();
    assert_eq!(load(&path).unwrap(), graph);
}

#[test]
fn an_untagged_node_saves_no_tag_key() {
    let graph: AnimationGraph = serde_json::from_str(TAGGED).unwrap();
    let idle = serde_json::to_string(graph.node("Idle").unwrap()).unwrap();
    assert!(!idle.contains("tag"), "{idle}");
    let aim = serde_json::to_string(graph.node("Aim").unwrap()).unwrap();
    assert!(aim.contains(r#""tag":"attack""#), "{aim}");
}
