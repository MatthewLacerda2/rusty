//! The `expression` op baked in a recipe: it agrees with `coordinates` and with the
//! equivalent `math` chain byte for byte, and refuses a miswired formula by name.

use serde_json::{json, Value};

use crate::bands::with_bands;
use crate::ops::test_graph::bits;
use crate::recipe::{Node, TextureRecipe};
use crate::{evaluate, Image};

/// The recipe of `nodes` (each `[id, op document, inputs]`), its last node baked at
/// 32×16.
pub(super) fn bake(nodes: &[(&str, Value, &[&str])]) -> Result<Image, String> {
    let parsed: Result<Vec<Node>, String> = nodes
        .iter()
        .map(|(id, op, inputs)| {
            let mut doc = op.clone();
            doc["id"] = json!(id);
            doc["inputs"] = json!(inputs);
            serde_json::from_value(doc).map_err(|e| e.to_string())
        })
        .collect();
    let recipe = TextureRecipe::new(0, parsed?).with_size([32, 16]);
    evaluate(&recipe).map_err(|e| e.to_string())
}

fn red(img: &Image) -> Vec<u32> {
    img.pixels().iter().map(|p| p[0].to_bits()).collect()
}

pub(super) fn expr(formula: &str) -> Value {
    json!({ "op": "expression", "expr": formula })
}

#[test]
fn u_and_v_are_what_coordinates_writes() {
    let coords = bake(&[("c", json!({ "op": "coordinates" }), &[])]).unwrap();
    let rg = json!({ "op": "expression", "r": "u", "g": "v" });
    let out = bake(&[("e", rg, &[])]).unwrap();
    assert_eq!(
        bits(&out),
        bits(&coords),
        "u in red, v in green, blue 0, alpha 1"
    );
    let gray = bake(&[("e", expr("u"), &[])]).unwrap();
    assert!(gray
        .pixels()
        .iter()
        .all(|p| p[0] == p[1] && p[1] == p[2] && p[3] == 1.0));
    assert_eq!(red(&gray), red(&coords));
}

fn math(func: &str, value: f32) -> Value {
    json!({ "op": "math", "func": func, "value": value })
}

#[test]
fn a_formula_bakes_what_its_math_chain_bakes() {
    let tau = std::f32::consts::TAU;
    let chain = bake(&[
        ("c", json!({ "op": "coordinates" }), &[]),
        ("m1", math("multiply", 8.0), &["c"]),
        ("m2", math("multiply", tau), &["m1"]),
        ("m3", math("sin", 0.0), &["m2"]),
        ("m4", math("multiply", 0.5), &["m3"]),
        ("m5", math("add", 0.5), &["m4"]),
    ])
    .unwrap();
    let formula = bake(&[("e", expr("sin(u * 8 * tau) * 0.5 + 0.5"), &[])]).unwrap();
    assert_eq!(red(&formula), red(&chain));
    // Through an input, with `math`'s two-input form.
    let noise = json!({ "op": "noise", "kind": "fbm", "scale": 4, "octaves": 3 });
    let wired = bake(&[
        ("n", noise.clone(), &[]),
        ("c", json!({ "op": "coordinates" }), &[]),
        ("m", json!({ "op": "math", "func": "atan2" }), &["n", "c"]),
    ])
    .unwrap();
    let named = bake(&[("n", noise, &[]), ("e", expr("atan2(a, u)"), &["n"])]).unwrap();
    assert_eq!(red(&named), red(&wired));
}

#[test]
fn a_nan_bakes_what_math_bakes() {
    let ten = json!({ "op": "constant", "color": [10.0, 10.0, 10.0, 1.0] });
    let chain = |tail: Option<Value>| {
        let mut nodes = vec![
            ("k", ten.clone(), &[][..]),
            ("p", math("power", 100.0), &["k"][..]),
            (
                "d",
                json!({ "op": "math", "func": "subtract" }),
                &["p", "p"][..],
            ),
        ];
        nodes.extend(tail.map(|t| ("s", t, &["d"][..])));
        bake(&nodes).unwrap()
    };
    let nan = chain(None);
    assert!(nan.pixels()[0][0].is_nan(), "inf − inf");
    let formula = bake(&[("e", expr("10 ^ 100 - 10 ^ 100"), &[])]).unwrap();
    assert_eq!(red(&formula), red(&nan));
    let rooted = chain(Some(math("sqrt", 0.0)));
    let formula = bake(&[("e", expr("sqrt(10 ^ 100 - 10 ^ 100)"), &[])]).unwrap();
    assert_eq!(red(&formula), red(&rooted));
}

#[test]
fn a_linked_k_bakes_like_its_number() {
    let k = json!({ "op": "constant", "color": [0.3, 0.3, 0.3, 1.0] });
    let number = json!({ "op": "expression", "expr": "u * k1 + k3", "k1": 0.3 });
    let linked = json!({ "op": "expression", "expr": "u * k1 + k3", "k1": { "node": "k" } });
    let a = bake(&[("k", k.clone(), &[]), ("e", number, &[])]).unwrap();
    let b = bake(&[("k", k, &[]), ("e", linked, &[])]).unwrap();
    assert_eq!(bits(&a), bits(&b));
}

#[test]
fn it_round_trips_and_bakes_the_same_in_any_band_count() {
    let doc = json!({ "id": "w", "op": "expression", "r": "fract(u * 3)", "b": "v",
                      "a_out": "0.5", "k2": 0.25 });
    let node: Node = serde_json::from_value(doc).unwrap();
    let again: Node = serde_json::from_value(serde_json::to_value(&node).unwrap()).unwrap();
    assert_eq!(again, node);
    let recipe = TextureRecipe::new(64, vec![node]);
    let one = with_bands(1, || evaluate(&recipe).unwrap());
    assert_eq!(
        bits(&with_bands(7, || evaluate(&recipe).unwrap())),
        bits(&one)
    );
    assert_eq!(one.pixels()[0][1], 0.0, "g left out reads expr's default 0");
    assert_eq!(one.pixels()[0][3], 0.5);
}
