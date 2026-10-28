//! Unit tests for the op-runner (`runner/mod.rs`, `runner/plan.rs`).

use super::*;
use crate::procgen::recipe::{Constant, Mix, Node, OpKind};

pub(super) fn node(id: &str, op: OpKind, inputs: &[&str]) -> Node {
    Node {
        id: id.into(),
        op,
        inputs: inputs.iter().map(|s| s.to_string()).collect(),
    }
}

pub(super) fn constant(id: &str, v: f32) -> Node {
    let color = [v, v, v, 1.0];
    node(id, OpKind::Constant(Constant { color }), &[])
}

#[test]
fn evaluates_in_dependency_order() {
    let c = node(
        "c",
        OpKind::Constant(Constant {
            color: [0.2, 0.4, 0.6, 1.0],
        }),
        &[],
    );
    let recipe = TextureRecipe::new(8, vec![c, node("inv", OpKind::Invert, &["c"])]);
    let p = evaluate(&recipe.with_output("inv")).unwrap().pixels()[0];
    assert!((p[0] - 0.8).abs() < 1e-5);
    assert!((p[1] - 0.6).abs() < 1e-5);
    assert!((p[2] - 0.4).abs() < 1e-5);
}

#[test]
fn missing_input_is_reported() {
    let recipe = TextureRecipe::new(8, vec![node("inv", OpKind::Invert, &["ghost"])]);
    assert!(matches!(
        evaluate(&recipe),
        Err(RunError::UnknownInput { .. })
    ));
}

#[test]
fn a_short_or_surplus_input_list_is_refused_naming_the_node() {
    let mix = OpKind::Mix(Mix {
        mode: crate::procgen::recipe::BlendMode::Add,
        factor: 1.0,
    });
    for (op, inputs) in [(mix, &["a"][..]), (OpKind::Invert, &["a", "a"][..])] {
        let recipe = TextureRecipe::new(8, vec![constant("a", 0.5), node("bad", op, inputs)]);
        let err = evaluate(&recipe).unwrap_err();
        assert!(matches!(err, RunError::Arity(_)), "{err:?}");
        assert!(err.to_string().contains("\"bad\""), "{err}");
    }
}

#[test]
fn a_duplicate_id_is_refused_by_name() {
    let recipe = TextureRecipe::new(8, vec![constant("a", 0.1), constant("a", 0.2)]);
    assert_eq!(evaluate(&recipe), Err(RunError::DuplicateId("a".into())));
}

#[test]
fn cycle_is_detected() {
    let recipe = TextureRecipe::new(
        8,
        vec![
            node("a", OpKind::Invert, &["b"]),
            node("b", OpKind::Invert, &["a"]),
        ],
    )
    .with_output("a");
    assert!(matches!(evaluate(&recipe), Err(RunError::Cycle(_))));
}

#[test]
fn empty_and_bad_resolution_error() {
    let empty = TextureRecipe::new(8, vec![]);
    assert_eq!(evaluate(&empty), Err(RunError::Empty));
    let bad = TextureRecipe::new(0, vec![node("c", OpKind::WhiteNoise, &[])]);
    assert_eq!(evaluate(&bad), Err(RunError::BadResolution));
}

/// A constant followed by `len` inverts, each reading the one before.
pub(super) fn chain(len: usize) -> TextureRecipe {
    let mut nodes = vec![constant("n0", 0.25)];
    for i in 1..=len {
        let prev = format!("n{}", i - 1);
        nodes.push(node(&format!("n{i}"), OpKind::Invert, &[&prev]));
    }
    TextureRecipe::new(4, nodes)
}

#[test]
fn a_long_chain_holds_a_constant_number_of_images() {
    let peak = |len| {
        let target = format!("n{len}");
        let plan = plan(&chain(len), &[&target], Scope::Needed).unwrap();
        run(&chain(len), &plan, &mut |_, _| {})
    };
    let (short, long) = (peak(4), peak(64));
    assert_eq!(short.peak_live, 2, "an input and its consumer");
    assert_eq!(long.peak_live, short.peak_live, "not one image per node");
    assert_eq!(
        long.images[0].pixels()[0][0],
        0.25,
        "64 inverts are the identity"
    );
}

#[test]
fn targets_are_kept_and_a_shared_input_is_freed_once() {
    // `a` feeds `x` twice and is also a target; `y` reads `x`.
    let mix = OpKind::Mix(Mix {
        mode: crate::procgen::recipe::BlendMode::Add,
        factor: 1.0,
    });
    let nodes = vec![
        constant("a", 0.25),
        node("x", mix, &["a", "a"]),
        node("y", OpKind::Invert, &["x"]),
    ];
    let recipe = TextureRecipe::new(4, nodes);
    let out = evaluate_many(&recipe, &["y", "a", "a"]).unwrap();
    let red: Vec<f32> = out.iter().map(|img| img.pixels()[0][0]).collect();
    assert_eq!(red, [0.5, 0.25, 0.25]);
}
