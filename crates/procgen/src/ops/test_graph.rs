use serde_json::{json, Value};

use crate::recipe::{Node, TextureRecipe};
use crate::{evaluate, Image};

pub fn node(id: &str, op: Value, inputs: &[&str]) -> Node {
    let mut doc = op.as_object().unwrap().clone();
    doc.insert("id".into(), json!(id));
    doc.insert("inputs".into(), json!(inputs));
    serde_json::from_value(Value::Object(doc)).unwrap()
}

pub fn bake(op: Value, wired: usize, extra: Vec<Node>, size: [u32; 2]) -> Image {
    let sources = [
        json!({ "op": "noise", "kind": "fbm", "scale": 4, "octaves": 3 }),
        json!({ "op": "voronoi", "scale": 5, "output": "cells" }),
        json!({ "op": "gradient", "kind": "radial" }),
        json!({ "op": "wave", "kind": "rings", "frequency": 3 }),
    ];
    // Past four inputs (`scatter` takes eight) the sources repeat.
    let ids = ["a", "b", "c", "d", "e", "f", "g", "h"];
    let mut nodes: Vec<Node> = (0..wired)
        .map(|i| node(ids[i], sources[i % sources.len()].clone(), &[]))
        .collect();
    nodes.extend(extra);
    nodes.push(node("out", op, &ids[..wired]));
    let recipe = TextureRecipe::new(0, nodes)
        .with_size(size)
        .with_seed(3)
        .with_output("out");
    evaluate(&recipe).unwrap()
}

pub fn bits(img: &Image) -> Vec<u32> {
    img.pixels().iter().flatten().map(|c| c.to_bits()).collect()
}
