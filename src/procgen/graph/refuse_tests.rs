//! Tests for what an instance refuses (#1744), each error naming what is wrong,
//! and the parse refusals.

use serde_json::{json, Value};

use super::tests::{graphs, recipe, wear};
use super::*;

/// A one-node recipe instancing `path` with `params` and `inputs`.
pub(super) fn instancing(path: &str, params: Value, inputs: &[&str]) -> Value {
    let mut nodes = vec![json!({ "id": "base", "op": "white_noise" })];
    nodes.push(
        json!({ "id": "w", "op": "graph", "path": path, "params": params, "inputs": inputs }),
    );
    json!({ "resolution": 8, "nodes": nodes })
}

pub(super) fn expand_err(doc: Value, entries: &[(&str, Value)]) -> String {
    expand(&recipe(doc), &graphs(entries)).unwrap_err()
}

fn wear_err(params: Value, inputs: &[&str]) -> String {
    let doc = instancing("wear.graph.json", params, inputs);
    expand_err(doc, &[("wear.graph.json", wear())])
}

/// Instancing `graph` as `wear.graph.json`, wired to `base`.
fn wear_err_in(params: Value, graph: Value) -> String {
    let doc = instancing("wear.graph.json", params, &["base"]);
    expand_err(doc, &[("wear.graph.json", graph)])
}

#[test]
fn a_param_value_is_checked_by_name() {
    let err = wear_err(json!({ "scale": "big" }), &["base"]);
    assert!(err.contains("\"scale\" takes a number"), "{err}");
    let err = wear_err(json!({ "scale": 64 }), &["base"]);
    assert!(err.contains("outside 1 to 32"), "{err}");
    let err = wear_err(json!({ "sclae": 2 }), &["base"]);
    assert!(
        err.contains("no param \"sclae\"") && err.contains("mode, scale"),
        "{err}"
    );
}

#[test]
fn an_instance_wires_exactly_the_graphs_inputs() {
    let err = wear_err(json!({}), &[]);
    assert!(err.contains("takes 1 inputs (base), but 0 given"), "{err}");
}

#[test]
fn a_missing_named_output_is_refused() {
    let mut doc = instancing("wear.graph.json", json!({}), &["base"]);
    doc["nodes"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "id": "o", "op": "invert", "inputs": ["w.rough"] }));
    let err = expand_err(doc, &[("wear.graph.json", wear())]);
    assert!(
        err.contains("no output \"rough\"") && err.contains("[height]"),
        "{err}"
    );
}

#[test]
fn an_error_inside_a_graph_names_the_prefixed_node_and_the_file() {
    let mut broken = wear();
    broken["nodes"][1]["inputs"] = json!(["base", "nowhere"]);
    let doc = instancing("wear.graph.json", json!({}), &["base"]);
    let flat = expand(&recipe(doc), &graphs(&[("wear.graph.json", broken)])).unwrap();
    let err = flat.validate().unwrap_err();
    assert!(
        err.contains("node \"w/blend\" (mix)") && err.contains("(in wear.graph.json)"),
        "{err}"
    );
}

#[test]
fn ids_inputs_and_placeholders_are_refused_at_parse() {
    let parse = |doc: Value| {
        serde_json::from_value::<TextureRecipe>(doc)
            .unwrap_err()
            .to_string()
    };
    let err = parse(json!({ "resolution": 8, "nodes": [{ "id": "a/b", "op": "white_noise" }] }));
    assert!(err.contains("no '/' or '.'"), "{err}");
    let err = parse(
        json!({ "resolution": 8, "nodes": [{ "id": "n", "op": "white_noise",
        "seed": { "param": "s" } }] }),
    );
    assert!(err.contains("names no param"), "{err}");
    let mut bad_default = wear();
    bad_default["params"]["scale"]["default"] = json!(99);
    let err = wear_err_in(json!({}), bad_default);
    assert!(
        err.contains("as its default") && err.contains("wear.graph.json"),
        "{err}"
    );
}

#[test]
fn a_graph_with_inputs_bakes_only_through_an_instance() {
    let err = expand(&recipe(wear()), &Graphs::new()).unwrap_err();
    assert!(err.contains("bakes only through an instance"), "{err}");
}
