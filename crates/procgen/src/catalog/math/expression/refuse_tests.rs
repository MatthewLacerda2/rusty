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
    // Whether `expr` is written decides it, not its text: "0" is refused as "0.0" is.
    for written in ["u", "0", "0.0"] {
        let rgb = json!({ "op": "expression", "expr": written, "r": "1", "g": "1", "b": "1" });
        assert!(refusal(&[("w", rgb, &[])]).contains("expr is never used"));
    }
    let rgb = json!({ "id": "w", "op": "expression", "r": "1", "g": "u", "b": "v" });
    let node: crate::recipe::Node = serde_json::from_value(rgb.clone()).unwrap();
    assert_eq!(
        serde_json::to_value(&node).unwrap()["expr"],
        Value::Null,
        "{rgb}"
    );
    let again = serde_json::to_value(&node).unwrap();
    assert_eq!(
        serde_json::from_value::<crate::recipe::Node>(again).unwrap(),
        node
    );
}

#[test]
fn the_inputs_wired_are_exactly_the_inputs_named() {
    let src = json!({ "op": "coordinates" });
    let unwired = refusal(&[("w", expr("a * b"), &[])]);
    assert!(
        unwired.contains("takes 2 inputs, but 0 given; the formulas name a, b with no input"),
        "{unwired}"
    );
    let one = refusal(&[("s", src.clone(), &[]), ("w", expr("a * b"), &["s"])]);
    assert!(one.contains("name b with no input wired"), "{one}");
    let unnamed = refusal(&[("s", src.clone(), &[]), ("w", expr("a"), &["s", "s"])]);
    assert!(
        unnamed.contains("but 2 given; no formula names the input wired as b"),
        "{unnamed}"
    );
    let five = ["s"; 5];
    let many = refusal(&[
        ("s", src.clone(), &[]),
        ("w", expr("a + b + c + d"), &five[..]),
    ]);
    assert!(many.contains("at most four inputs"), "{many}");
    let gap = refusal(&[("w", expr("a + c"), &[])]);
    assert!(gap.contains("name input c but not b"), "{gap}");
    let ok = bake(&[("s", src, &[]), ("w", expr("a.g + b"), &["s", "s"])]);
    assert!(ok.is_ok(), "{ok:?}");
}
