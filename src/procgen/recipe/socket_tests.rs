//! The `socket` flag (#1494) held to the parser and to [`OpKind::sockets`].

use serde_json::{json, Value};

use super::*;
use crate::procgen::recipe::{sample_op, Link, OpKind};

const LINK: fn() -> Value = || json!({ "node": "m", "min": 0.25, "max": 2.0 });

/// `info`'s op with param `name` set to `value`, parsed by serde.
fn with_param(info: &OpInfo, name: &str, value: Value) -> Result<OpKind, serde_json::Error> {
    let Value::Object(mut doc) = serde_json::to_value(sample_op(info)).unwrap() else {
        unreachable!()
    };
    doc.insert(name.into(), value);
    serde_json::from_value(Value::Object(doc))
}

#[test]
fn a_param_takes_a_link_exactly_when_the_catalog_says_socket() {
    for info in OPS {
        for p in info.params {
            let parsed = with_param(info, p.name, LINK());
            assert_eq!(
                parsed.is_ok(),
                p.socket,
                "{}.{}: {parsed:?}",
                info.op,
                p.name
            );
        }
    }
}

#[test]
fn only_amount_like_numbers_are_socketable() {
    for p in OPS.iter().flat_map(|info| info.params).filter(|p| p.socket) {
        assert_eq!(p.ty, "number", "{} is socketable but not a number", p.name);
    }
}

#[test]
fn every_socket_param_reaches_the_runner_as_a_link() {
    // `OpKind::sockets` has a `_ => vec![]` arm, so the compiler won't flag a socket
    // param missing from it: this does.
    let want = Link {
        node: "m".into(),
        min: 0.25,
        max: 2.0,
    };
    for info in OPS {
        let names: Vec<&str> = info
            .params
            .iter()
            .filter(|p| p.socket)
            .map(|p| p.name)
            .collect();
        let sockets: Vec<&str> = sample_op(info).sockets().iter().map(|s| s.0).collect();
        assert_eq!(sockets, names, "{}: sockets() vs the catalog", info.op);
        for name in names {
            let op = with_param(info, name, LINK()).unwrap();
            assert_eq!(op.links(), vec![(name, &want)], "{}.{name}", info.op);
        }
    }
}

#[test]
fn a_linked_param_round_trips_through_json() {
    let info = OPS.iter().find(|i| i.op == "warp").unwrap();
    let op = with_param(info, "strength", LINK()).unwrap();
    let back: OpKind = serde_json::from_value(serde_json::to_value(&op).unwrap()).unwrap();
    assert_eq!(back, op);
}
