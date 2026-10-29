//! The catalog property behind linkable params (#1494): for every socketable param,
//! a link to a `constant` node of value v (min 0, max 1) bakes exactly like the
//! number v, square and non-square.

use serde_json::{json, Value};

use crate::procgen::recipe::{sample_op, Node, OpInfo, TextureRecipe, OPS};
use crate::procgen::{evaluate, Image};

/// A node parsed from `id` and the op document `op`.
fn node(id: &str, op: Value, inputs: &[&str]) -> Node {
    let mut doc = op.as_object().unwrap().clone();
    doc.insert("id".into(), json!(id));
    doc.insert("inputs".into(), json!(inputs));
    serde_json::from_value(Value::Object(doc)).unwrap()
}

/// `info`'s sample op with `param` set to `value`, fed by busy inputs, baked at
/// `size`. The constant `k` rides along either way, so both recipes hold the same
/// nodes.
fn bake(info: &OpInfo, param: &str, value: Value, v: f32, size: [u32; 2]) -> Image {
    let mut op = serde_json::to_value(sample_op(info)).unwrap();
    op[param] = value;
    let sources = [
        json!({ "op": "noise", "kind": "fbm", "scale": 4, "octaves": 3 }),
        json!({ "op": "voronoi", "scale": 5, "output": "cells" }),
        json!({ "op": "gradient", "kind": "radial" }),
    ];
    let ids = ["a", "b", "c"];
    let wired = sample_op(info).arity().max as usize;
    let mut nodes: Vec<Node> = (0..wired)
        .map(|i| node(ids[i], sources[i].clone(), &[]))
        .collect();
    nodes.push(node(
        "k",
        json!({ "op": "constant", "color": [v, v, v, 1.0] }),
        &[],
    ));
    nodes.push(node("out", op, &ids[..wired]));
    let recipe = TextureRecipe::new(0, nodes)
        .with_size(size)
        .with_seed(3)
        .with_output("out");
    evaluate(&recipe).unwrap()
}

#[test]
fn a_link_to_a_constant_bakes_exactly_like_its_number() {
    let mut checked = 0;
    for info in OPS {
        for p in info.params.iter().filter(|p| p.socket) {
            for v in [0.0, 0.15, 0.6] {
                for size in [[32, 32], [64, 16]] {
                    let number = bake(info, p.name, json!(v), v, size);
                    let link = json!({ "node": "k" });
                    let linked = bake(info, p.name, link, v, size);
                    let bits = |img: &Image| -> Vec<u32> {
                        img.pixels().iter().flatten().map(|c| c.to_bits()).collect()
                    };
                    let identical = bits(&number) == bits(&linked);
                    assert!(identical, "{}.{} = {v} at {size:?}", info.op, p.name);
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 18 * 3 * 2, "every socketable param is checked");
}
