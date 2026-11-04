//! The catalog property behind linkable params (#1494): for every socketable param,
//! a link to a `constant` node of value v (min 0, max 1) bakes exactly like the
//! number v, square and non-square.

use serde_json::{json, Value};

use super::test_graph::{bake as bake_op, bits, node};
use crate::procgen::recipe::{sample_op, OpInfo, OPS};
use crate::procgen::Image;

/// `info`'s sample op with `param` set to `value`, fed by busy inputs, baked at
/// `size`. The constant `k` rides along either way, so both recipes hold the same
/// nodes.
fn bake(info: &OpInfo, param: &str, value: Value, v: f32, size: [u32; 2]) -> Image {
    let mut op = serde_json::to_value(sample_op(info)).unwrap();
    op[param] = value;
    let wired = sample_op(info).arity().max as usize;
    let k = node(
        "k",
        json!({ "op": "constant", "color": [v, v, v, 1.0] }),
        &[],
    );
    bake_op(op, wired, vec![k], size)
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
                    let identical = bits(&number) == bits(&linked);
                    assert!(identical, "{}.{} = {v} at {size:?}", info.op, p.name);
                    checked += 1;
                }
            }
        }
    }
    // Derived from the catalog, so a branch adding a socket param edits no count.
    let sockets = OPS.iter().flat_map(|i| i.params).filter(|p| p.socket);
    assert_eq!(
        checked,
        sockets.count() * 3 * 2,
        "every socketable param is checked"
    );
    assert!(checked > 0, "the catalog has socketable params to check");
}
