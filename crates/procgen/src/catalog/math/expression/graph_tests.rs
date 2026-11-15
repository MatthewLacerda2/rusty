//! A formula as a graph's `text` param: `bind` fills `expr` before the node parses,
//! so an instance bakes the formula it passes, and a typo in one is refused.

use serde_json::{json, Value};

use super::tests::{bake, expr};
use crate::graph::{expand, Graphs};
use crate::{evaluate, TextureRecipe};

/// A graph whose one node's formula is the param `f`, `u` by default.
fn ramp() -> TextureRecipe {
    serde_json::from_value(json!({
        "resolution": 8,
        "params": { "f": { "type": "text", "default": "u" } },
        "nodes": [ { "id": "e", "op": "expression", "expr": { "param": "f" } } ]
    }))
    .unwrap()
}

/// A recipe instancing `ramp`, with `params`, baked at 32×16.
fn instance(params: Value) -> Result<crate::Image, String> {
    let graphs: Graphs = [("ramp.graph.json".to_string(), ramp())]
        .into_iter()
        .collect();
    let parent: TextureRecipe = serde_json::from_value(json!({
        "resolution": 32, "size": [32, 16],
        "nodes": [ { "id": "r", "op": "graph", "path": "ramp.graph.json", "params": params } ]
    }))
    .unwrap();
    let flat = expand(&parent, &graphs).map_err(|e| e.to_string())?.recipe;
    evaluate(&flat).map_err(|e| e.to_string())
}

#[test]
fn an_instance_bakes_the_formula_its_param_passes() {
    let passed = instance(json!({ "f": "fract(v * 3)" })).unwrap();
    assert_eq!(
        passed.pixels(),
        bake(&[("e", expr("fract(v * 3)"), &[])]).unwrap().pixels()
    );
    let default = instance(json!({})).unwrap();
    assert_eq!(
        default.pixels(),
        bake(&[("e", expr("u"), &[])]).unwrap().pixels()
    );
}

#[test]
fn a_typo_in_a_passed_formula_is_refused() {
    let e = instance(json!({ "f": "sine(u)" })).expect_err("refused");
    assert!(e.contains(r#"unknown function "sine""#), "{e}");
}
