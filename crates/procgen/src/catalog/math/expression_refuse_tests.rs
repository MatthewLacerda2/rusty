//! The `expression` op's refusals: each names the node, the field, the column and
//! the fix, and the inputs wired must be exactly the inputs the formulas name.

use serde_json::{json, Value};

use super::tests::{bake, expr};

fn refusal(nodes: &[(&str, Value, &[&str])]) -> String {
    bake(nodes).expect_err("refused")
}

#[test]
fn refusals_name_the_node_the_field_the_column_and_the_fix() {
    let e = refusal(&[("w", expr("0.5 + 0.5 * sine(u)"), &[])]);
    assert!(
        e.contains(r#"node "w" (expression): unknown function "sine" at column 13 of expr; functions are min, max"#),
        "{e}"
    );
    let e = refusal(&[("w", json!({ "op": "expression", "a_out": "u +" }), &[])]);
    assert!(
        e.contains("the formula ends early at column 4 of a_out; expected"),
        "{e}"
    );
    let rgb = json!({ "op": "expression", "expr": "u", "r": "1", "g": "1", "b": "1" });
    assert!(refusal(&[("w", rgb, &[])]).contains("expr is never used"));
}

#[test]
fn the_inputs_wired_are_exactly_the_inputs_named() {
    let src = json!({ "op": "coordinates" });
    let unwired = refusal(&[("w", expr("a * b"), &[])]);
    assert!(unwired.contains("takes 2 inputs, but 0 given"), "{unwired}");
    let unnamed = refusal(&[("s", src.clone(), &[]), ("w", expr("u"), &["s"])]);
    assert!(unnamed.contains("takes 0 inputs, but 1 given"), "{unnamed}");
    let gap = refusal(&[("w", expr("a + c"), &[])]);
    assert!(gap.contains("name input c but not b"), "{gap}");
    let ok = bake(&[("s", src, &[]), ("w", expr("a.g + b"), &["s", "s"])]);
    assert!(ok.is_ok(), "{ok:?}");
}
