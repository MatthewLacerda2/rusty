//! Tests for checking the whole recipe and for `evaluate_each` (#1486).

use super::tests::{chain, constant, node};
use super::*;
use crate::procgen::recipe::OpKind;

#[test]
fn a_cycle_the_output_never_reaches_is_still_refused() {
    let recipe = TextureRecipe::new(
        8,
        vec![
            node("a", OpKind::Invert, &["b"]),
            node("b", OpKind::Invert, &["a"]),
            constant("out", 0.5),
        ],
    )
    .with_output("out");
    assert_eq!(evaluate(&recipe), Err(RunError::Cycle("a".into())));
}

#[test]
fn evaluate_each_sees_every_node_once_inputs_first() {
    // `dead` feeds nothing the output reads, and is still evaluated.
    let recipe = TextureRecipe::new(
        4,
        vec![
            node("inv", OpKind::Invert, &["c"]),
            constant("c", 0.25),
            node("dead", OpKind::Invert, &["c"]),
        ],
    )
    .with_output("inv");
    let mut seen = Vec::new();
    evaluate_each(&recipe, &Sources::new(), |i, img| {
        seen.push((i, img.pixels()[0][0]))
    })
    .unwrap();
    assert_eq!(seen, [(1, 0.25), (0, 0.75), (2, 0.75)]);
}

#[test]
fn evaluate_each_holds_a_constant_number_of_images() {
    let recipe = chain(64);
    let plan = plan(&recipe, &[], Scope::Every).unwrap();
    let mut count = 0;
    let run = run(&recipe, &Sources::new(), &plan, &mut |_, _| count += 1);
    assert_eq!(count, 65);
    assert_eq!(
        run.peak_live, 2,
        "an input and its consumer, not one per node"
    );
    assert!(run.images.is_empty(), "no targets kept");
}

/// Ten constant leaves listed first, then a left-deep `mix` chain over them.
fn leaves_first() -> TextureRecipe {
    let mix = || {
        OpKind::Mix(crate::procgen::recipe::Mix {
            mode: crate::procgen::recipe::BlendMode::Add,
            factor: 1.0,
        })
    };
    let mut nodes: Vec<_> = (0..10).map(|i| constant(&format!("l{i}"), 0.0)).collect();
    nodes.push(node("m1", mix(), &["l0", "l1"]));
    for i in 2..10 {
        let (prev, leaf) = (format!("m{}", i - 1), format!("l{i}"));
        nodes.push(node(&format!("m{i}"), mix(), &[&prev, &leaf]));
    }
    TextureRecipe::new(4, nodes)
}

#[test]
fn leaves_listed_first_still_run_beside_their_consumer() {
    let recipe = leaves_first();
    for (scope, targets) in [(Scope::Needed, &["m9"][..]), (Scope::Every, &[][..])] {
        let plan = plan(&recipe, targets, scope).unwrap();
        let run = run(&recipe, &Sources::new(), &plan, &mut |_, _| {});
        assert_eq!(
            run.peak_live, 3,
            "two inputs and their mix, not every leaf at once"
        );
    }
}

#[test]
fn evaluate_each_refuses_a_misspelled_declared_output() {
    let out = TextureRecipe::new(4, vec![constant("c", 0.5)]).with_output("ghost");
    let mut slots = out.clone();
    slots.output = None;
    slots.outputs.insert("base_color".into(), "gohst".into());
    let err = |r: &TextureRecipe| evaluate_each(r, &Sources::new(), |_, _| {});
    assert_eq!(err(&out), Err(RunError::UnknownOutput("ghost".into())));
    assert_eq!(err(&slots), Err(RunError::UnknownOutput("gohst".into())));
}
