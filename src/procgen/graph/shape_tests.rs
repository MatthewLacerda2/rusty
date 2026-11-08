//! `rewrite` finds a node's links and seed by their JSON shape (#1744): a link is a
//! top-level object's `node`, the seed a top-level `seed`. These hold that shape to
//! the typed accessors for every catalog row, so an op that nests a link deeper or
//! keeps its seed under another key fails here the day it is added, and "a new op
//! needs nothing for graphs" (`src/procgen/CLAUDE.md`) stays true.

use serde_json::{json, Map, Value};

use super::expand::{link_refs, seed_of};
use crate::procgen::recipe::{sample_op, Node, OpInfo, OpKind, OPS};

/// `info`'s sample op with `set` written over it, as a node's JSON, and parsed.
fn node(info: &OpInfo, set: &[(&str, Value)]) -> Option<(Map<String, Value>, OpKind)> {
    let Value::Object(mut doc) = serde_json::to_value(sample_op(info)).unwrap() else {
        unreachable!()
    };
    for (k, v) in set {
        doc.insert(k.to_string(), v.clone());
    }
    let op: OpKind = serde_json::from_value(Value::Object(doc)).ok()?;
    let node = Node {
        id: "n".into(),
        op: op.clone(),
        inputs: Vec::new(),
    };
    let Value::Object(map) = serde_json::to_value(node).unwrap() else {
        unreachable!()
    };
    Some((map, op))
}

#[test]
fn every_link_is_where_rewrite_looks() {
    for info in OPS {
        let links: Vec<(&str, Value)> = info
            .params
            .iter()
            .filter(|p| p.socket)
            .map(|p| (p.name, json!({ "node": format!("m_{}", p.name) })))
            .collect();
        let (mut map, op) = node(info, &links).expect("every socket takes a link");
        let mut found: Vec<String> = link_refs(&mut map).map(|r| r.clone()).collect();
        let mut typed: Vec<String> = op.links().iter().map(|(_, l)| l.node.clone()).collect();
        found.sort();
        typed.sort();
        assert_eq!(found, typed, "{}: links by shape vs OpKind::links", info.op);
    }
}

#[test]
fn every_seed_is_where_rewrite_looks() {
    for info in OPS {
        let (map, op) = node(info, &[]).unwrap();
        assert_eq!(seed_of(&map).unwrap_or(0), op.seed(), "{}", info.op);
        // Any integer param that drives `Op::seed` must be the top-level `seed`.
        for p in info.params.iter().filter(|p| p.ty == "integer") {
            if let Some((map, op)) = node(info, &[(p.name, json!(7))]) {
                let found = seed_of(&map).unwrap_or(0);
                assert_eq!(
                    found,
                    op.seed(),
                    "{}.{}: seed by shape vs Op::seed",
                    info.op,
                    p.name
                );
            }
        }
    }
}
