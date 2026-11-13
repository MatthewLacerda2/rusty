//! Tests for nested instances (#1744): ids prefixed at each level, params passed
//! down, cycles named by their chain, and the depth cap.

use serde_json::{json, Value};

use super::refuse_tests::{expand_err, instancing};
use super::tests::{graphs, recipe, wear};
use super::*;

#[test]
fn nested_instances_prefix_each_level() {
    let outer = json!({
        "resolution": 8, "inputs": ["base"],
        "nodes": [{ "id": "inner", "op": "graph", "path": "wear.graph.json", "inputs": ["base"] }]
    });
    let doc = instancing("outer.graph.json", json!({}), &["base"]);
    let entries = [("outer.graph.json", outer), ("wear.graph.json", wear())];
    let flat = expand(&recipe(doc), &graphs(&entries)).unwrap();
    let ids: Vec<&str> = flat.recipe.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, ["base", "w/inner/grain", "w/inner/blend"]);
    assert_eq!(flat.recipe.output.as_deref(), Some("w/inner/blend"));
    assert_eq!(flat.file_of("w/inner/grain"), Some("wear.graph.json"));
    assert!(flat.validate().is_ok());
}

#[test]
fn a_cycle_names_the_chain_and_depth_is_capped() {
    let selfish = |path: &str, next: &str| {
        let node = json!({ "id": "again", "op": "graph", "path": next });
        (
            path.to_string(),
            json!({ "resolution": 8, "nodes": [node] }),
        )
    };
    let (a, b) = (
        selfish("a.graph.json", "b.graph.json"),
        selfish("b.graph.json", "a.graph.json"),
    );
    let entries = [(a.0.as_str(), a.1), (b.0.as_str(), b.1)];
    let err = expand_err(instancing("a.graph.json", json!({}), &[]), &entries);
    assert!(
        err.contains("a.graph.json → b.graph.json → a.graph.json"),
        "{err}"
    );

    let deep: Vec<(String, Value)> = (0..=MAX_DEPTH)
        .map(|i| {
            selfish(
                &format!("g{i}.graph.json"),
                &format!("g{}.graph.json", i + 1),
            )
        })
        .collect();
    let entries: Vec<(&str, Value)> = deep.iter().map(|(p, d)| (p.as_str(), d.clone())).collect();
    let err = expand_err(instancing("g0.graph.json", json!({}), &[]), &entries);
    assert!(
        err.contains(&format!("more than {MAX_DEPTH} deep")),
        "{err}"
    );
}

#[test]
fn an_instance_placeholder_passes_the_outer_params_value_down() {
    let mut outer = json!({
        "resolution": 8, "inputs": ["base"],
        "params": { "s": { "type": "number", "default": 9 } },
        "nodes": [{ "id": "inner", "op": "graph", "path": "wear.graph.json",
                    "params": { "scale": { "param": "s" } }, "inputs": ["base"] }]
    });
    let doc = instancing("outer.graph.json", json!({ "s": 12 }), &["base"]);
    let entries = [
        ("outer.graph.json", outer.clone()),
        ("wear.graph.json", wear()),
    ];
    let flat = expand(&recipe(doc), &graphs(&entries)).unwrap().recipe;
    let OpKind::Noise(n) = &flat.nodes[1].op else {
        panic!()
    };
    assert_eq!(n.scale, 12.0);
    outer["nodes"][0]["params"]["scale"] = json!({ "param": "nope" });
    let doc = instancing("outer.graph.json", json!({}), &["base"]);
    let err = expand_err(
        doc,
        &[("outer.graph.json", outer), ("wear.graph.json", wear())],
    );
    assert!(
        err.contains("names no param of the graph") && err.contains("outer.graph.json"),
        "{err}"
    );
}
