//! Test support: a catalog op baked in a small graph, its inputs wired to busy
//! sources. The catalog-wide bake tests (`link_tests`, `seed_tests`) build on it,
//! so a new op is baked with nothing added to them.

use serde_json::{json, Value};

use crate::recipe::{Node, TextureRecipe};
use crate::{evaluate, Image};

/// A node parsed from `id` and the op document `op`.
pub fn node(id: &str, op: Value, inputs: &[&str]) -> Node {
    let mut doc = op.as_object().unwrap().clone();
    doc.insert("id".into(), json!(id));
    doc.insert("inputs".into(), json!(inputs));
    serde_json::from_value(Value::Object(doc)).unwrap()
}

/// The op document `op` as node `out`, its first `wired` inputs fed by busy
/// sources, plus the `extra` nodes (which nothing wires), baked at `size` with
/// recipe seed 3.
pub fn bake(op: Value, wired: usize, extra: Vec<Node>, size: [u32; 2]) -> Image {
    let sources = [
        json!({ "op": "noise", "kind": "fbm", "scale": 4, "octaves": 3 }),
        json!({ "op": "voronoi", "scale": 5, "output": "cells" }),
        json!({ "op": "gradient", "kind": "radial" }),
    ];
    let ids = ["a", "b", "c"];
    let mut nodes: Vec<Node> = (0..wired)
        .map(|i| node(ids[i], sources[i].clone(), &[]))
        .collect();
    nodes.extend(extra);
    nodes.push(node("out", op, &ids[..wired]));
    let recipe = TextureRecipe::new(0, nodes)
        .with_size(size)
        .with_seed(3)
        .with_output("out");
    evaluate(&recipe).unwrap()
}

/// An image's pixels as bit patterns, so two bakes compare exactly.
pub fn bits(img: &Image) -> Vec<u32> {
    img.pixels().iter().flatten().map(|c| c.to_bits()).collect()
}
