//! Tests for what the review of #1744 found slipping through: ids shared with an
//! instance, a placeholder path, a seed that isn't a `u32`, a graph param named
//! `param`, errors naming their file, and the node cap.

use serde_json::{json, Value};

use super::refuse_tests::{expand_err, instancing};
use super::tests::{graphs, recipe, wear};
use super::*;

fn wear_entries() -> [(&'static str, Value); 1] {
    [("wear.graph.json", wear())]
}

#[test]
fn an_id_shared_with_an_instance_is_refused_in_every_scope() {
    let mut doc = instancing("wear.graph.json", json!({}), &["base"]);
    doc["nodes"][0]["id"] = json!("w");
    let err = expand_err(doc.clone(), &wear_entries());
    assert!(err.contains("two nodes share the id 'w'"), "{err}");
    // Two instances under one id.
    doc["nodes"][0] = doc["nodes"][1].clone();
    let err = expand_err(doc, &wear_entries());
    assert!(err.contains("two nodes share the id 'w'"), "{err}");
    // Inside a graph, the prefixed id and the file are named.
    let mut twice = wear();
    twice["nodes"][1]["id"] = json!("grain");
    let doc = instancing("wear.graph.json", json!({}), &["base"]);
    let err = expand_err(doc, &[("wear.graph.json", twice)]);
    assert!(
        err.contains("'w/grain'") && err.contains("(in wear.graph.json)"),
        "{err}"
    );
}

#[test]
fn an_instance_path_takes_no_placeholder() {
    let doc = json!({ "resolution": 8, "params": { "p": { "type": "text", "default": "a.graph.json" } },
        "nodes": [{ "id": "g", "op": "graph", "path": { "param": "p" } }] });
    let err = serde_json::from_value::<TextureRecipe>(doc)
        .unwrap_err()
        .to_string();
    assert!(err.contains("path is a file, never a param"), "{err}");
}

#[test]
fn an_inner_seed_that_is_not_a_u32_is_refused_whatever_the_instance_seed() {
    let graph = json!({ "resolution": 8,
        "params": { "s": { "type": "number", "default": 3 } },
        "nodes": [{ "id": "n", "op": "white_noise", "seed": { "param": "s" } }] });
    for seed in [0, 5] {
        let mut doc = instancing("g.graph.json", json!({ "s": 3.5 }), &[]);
        doc["nodes"][1]["seed"] = json!(seed);
        let err = expand_err(doc, &[("g.graph.json", graph.clone())]);
        assert!(
            err.contains("\"w/n\"") && err.contains("expected u32"),
            "{seed}: {err}"
        );
    }
}

#[test]
fn a_graph_param_named_param_is_a_value_not_a_placeholder() {
    let mut graph = wear();
    graph["params"] = json!({ "param": { "type": "text", "default": "multiply" } });
    graph["nodes"][0]["scale"] = json!(4);
    graph["nodes"][1]["mode"] = json!({ "param": "param" });
    let doc = instancing("wear.graph.json", json!({ "param": "add" }), &["base"]);
    let flat = expand(&recipe(doc), &graphs(&[("wear.graph.json", graph)])).unwrap();
    let blend = serde_json::to_value(&flat.recipe.nodes[2]).unwrap();
    assert_eq!(blend["mode"], json!("add"));
}

#[test]
fn an_unknown_output_inside_a_graph_names_its_file() {
    let mut ghost = wear();
    ghost["output"] = json!("ghost");
    let doc = instancing("wear.graph.json", json!({}), &["base"]);
    let flat = expand(&recipe(doc), &graphs(&[("wear.graph.json", ghost)])).unwrap();
    let err = flat.validate().unwrap_err();
    assert!(
        err.contains("'w/ghost'") && err.contains("(in wear.graph.json)"),
        "{err}"
    );
}

#[test]
fn an_expansion_past_the_node_cap_is_refused_with_its_chain() {
    let side = 300; // 300 instances of 300 nodes: 90 000 > MAX_NODES.
    assert!(side * side > MAX_NODES);
    let leaf: Vec<Value> = (0..side)
        .map(|i| json!({ "id": format!("n{i}"), "op": "white_noise" }))
        .collect();
    let top: Vec<Value> = (0..side)
        .map(|i| json!({ "id": format!("g{i}"), "op": "graph", "path": "leaf.graph.json" }))
        .collect();
    let leaf = json!({ "resolution": 8, "nodes": leaf });
    let err = expand_err(
        json!({ "resolution": 8, "nodes": top }),
        &[("leaf.graph.json", leaf)],
    );
    assert!(
        err.contains("passes 65536 nodes") && err.contains("inside leaf.graph.json"),
        "{err}"
    );
}
