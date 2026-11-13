//! Tests for expanding graph instances (#1744): byte-identical to hand inlining,
//! seeds, outputs, and what a stored recipe keeps.

use serde_json::{json, Value};

use super::*;
use crate::evaluate;

/// A graph reading one input: a noise whose `gain` links to it, mixed onto it.
pub(super) fn wear() -> Value {
    json!({
        "resolution": 8,
        "params": {
            "scale": { "type": "number", "default": 4, "min": 1, "max": 32 },
            "mode": { "type": "text", "default": "multiply" }
        },
        "inputs": ["base"],
        "nodes": [
            { "id": "grain", "op": "noise", "kind": "fbm", "scale": { "param": "scale" },
              "octaves": 3, "gain": { "node": "base" } },
            { "id": "blend", "op": "mix", "mode": { "param": "mode" }, "inputs": ["base", "grain"] }
        ],
        "output": "blend",
        "outputs": { "height": "grain" }
    })
}

pub(super) fn recipe(doc: Value) -> TextureRecipe {
    serde_json::from_value(doc).unwrap()
}

pub(super) fn graphs(entries: &[(&str, Value)]) -> Graphs {
    let parsed = entries
        .iter()
        .map(|(p, d)| (p.to_string(), recipe(d.clone())));
    parsed.collect()
}

/// A parent instancing `wear` at `seed`, reading both its outputs.
fn parent(seed: u32) -> Value {
    json!({
        "resolution": 32, "seed": 7,
        "nodes": [
            { "id": "base", "op": "white_noise", "seed": 2 },
            { "id": "wear", "op": "graph", "path": "wear.graph.json",
              "params": { "scale": 6 }, "seed": seed, "inputs": ["base"] },
            { "id": "out", "op": "mix", "mode": "add", "inputs": ["wear", "wear.height"] }
        ]
    })
}

fn expanded(seed: u32) -> TextureRecipe {
    let g = graphs(&[("wear.graph.json", wear())]);
    expand(&recipe(parent(seed)), &g).unwrap().recipe
}

#[test]
fn an_instance_at_seed_zero_is_its_nodes_pasted_by_hand() {
    let pasted = recipe(json!({
        "resolution": 32, "seed": 7,
        "nodes": [
            { "id": "base", "op": "white_noise", "seed": 2 },
            { "id": "wear_grain", "op": "noise", "kind": "fbm", "scale": 6, "octaves": 3,
              "gain": { "node": "base" } },
            { "id": "wear_blend", "op": "mix", "mode": "multiply", "inputs": ["base", "wear_grain"] },
            { "id": "out", "op": "mix", "mode": "add", "inputs": ["wear_blend", "wear_grain"] }
        ]
    }));
    let flat = expanded(0);
    let ids: Vec<&str> = flat.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, ["base", "wear/grain", "wear/blend", "out"]);
    // A `/` id parses nowhere, so the hand-pasted ids say `_`: same nodes otherwise.
    let renamed = flat.to_json().unwrap().replace("wear/", "wear_");
    assert_eq!(recipe(serde_json::from_str(&renamed).unwrap()), pasted);
    let hand = evaluate(&pasted).unwrap();
    assert_eq!(
        evaluate(&flat).unwrap().pixels(),
        hand.pixels(),
        "byte-identical bake"
    );
}

#[test]
fn two_instance_seeds_bake_differently() {
    let (a, b) = (expanded(1), expanded(2));
    assert_ne!(
        evaluate(&a).unwrap().pixels(),
        evaluate(&b).unwrap().pixels()
    );
    assert_ne!(
        evaluate(&a).unwrap().pixels(),
        evaluate(&expanded(0)).unwrap().pixels()
    );
}

#[test]
fn a_recipe_without_instances_expands_to_itself() {
    let plain = recipe(json!({ "resolution": 8, "nodes": [{ "id": "w", "op": "white_noise" }] }));
    assert_eq!(expand(&plain, &Graphs::new()).unwrap().recipe, plain);
}

#[test]
fn the_stored_recipe_keeps_the_instance_and_the_graph_its_placeholders() {
    let parent = recipe(parent(3));
    let back: TextureRecipe = serde_json::from_str(&parent.to_json().unwrap()).unwrap();
    assert_eq!(back, parent);
    assert!(matches!(back.nodes[1].op, OpKind::Graph(ref g) if g.seed == 3));
    let wear = recipe(wear());
    let json = wear.to_json().unwrap();
    assert!(json.contains(r#""param": "scale""#), "{json}");
    assert_eq!(recipe(serde_json::from_str(&json).unwrap()), wear);
}

#[test]
fn a_graph_baked_alone_uses_its_defaults() {
    let mut alone = wear();
    let alone = alone.as_object_mut().unwrap();
    alone.remove("output");
    alone.remove("outputs");
    alone["inputs"] = json!([]);
    alone["nodes"] =
        json!([{ "id": "n", "op": "noise", "kind": "fbm", "scale": { "param": "scale" } }]);
    let r = recipe(Value::Object(alone.clone()));
    let OpKind::Noise(n) = &r.nodes[0].op else {
        panic!()
    };
    assert_eq!(n.scale, 4.0);
    assert!(evaluate(&expand(&r, &Graphs::new()).unwrap().recipe).is_ok());
}

#[test]
fn the_runner_refuses_an_unexpanded_instance() {
    let err = evaluate(&recipe(parent(0))).unwrap_err().to_string();
    assert!(
        err.contains("\"wear\" (graph)") && err.contains("never expanded"),
        "{err}"
    );
}
