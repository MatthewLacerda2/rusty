//! Per-op correctness: generators + color ops (#270). The math/vector/filter ops are
//! covered in `procgen_ops_math.rs`. Each op is exercised through the public runner
//! (`evaluate`) on a tiny recipe, so the test pins observable pixel behaviour.

use rusty::procgen::recipe::{BlendMode, GradientKind, Node, OpKind, TextureRecipe};
use rusty::procgen::{evaluate, Image};

/// Build a recipe from `nodes` with `output` and evaluate it.
fn run(nodes: Vec<Node>, output: &str, resolution: u32) -> Image {
    let recipe = TextureRecipe {
        resolution,
        seed: 0,
        nodes,
        output: Some(output.into()),
    };
    evaluate(&recipe).expect("recipe evaluates")
}

fn node(id: &str, op: OpKind, inputs: &[&str]) -> Node {
    Node {
        id: id.into(),
        op,
        inputs: inputs.iter().map(|s| s.to_string()).collect(),
    }
}

fn constant(id: &str, c: [f32; 4]) -> Node {
    node(id, OpKind::Constant { color: c }, &[])
}

#[test]
fn constant_fills_a_flat_color() {
    let img = run(vec![constant("c", [0.1, 0.2, 0.3, 1.0])], "c", 8);
    assert!(img.pixels().iter().all(|p| *p == [0.1, 0.2, 0.3, 1.0]));
}

#[test]
fn checker_alternates_neighbouring_tiles() {
    // 2 tiles over an 8px canvas -> 4px squares; (0,0) and (4,0) differ.
    let img = run(
        vec![node(
            "k",
            OpKind::Checker {
                tiles: 2,
                color_a: [0.0, 0.0, 0.0, 1.0],
                color_b: [1.0, 1.0, 1.0, 1.0],
            },
            &[],
        )],
        "k",
        8,
    );
    assert_ne!(img.get_wrapped(0, 0), img.get_wrapped(4, 0));
}

#[test]
fn linear_gradient_ramps_left_to_right() {
    let img = run(
        vec![node(
            "g",
            OpKind::Gradient {
                kind: GradientKind::Linear,
            },
            &[],
        )],
        "g",
        16,
    );
    let left = img.get_wrapped(0, 0)[0];
    let right = img.get_wrapped(15, 0)[0];
    assert!(right > left, "gradient must increase: {left} -> {right}");
}

#[test]
fn invert_complements_rgb() {
    let img = run(
        vec![
            constant("c", [0.25, 0.5, 0.75, 1.0]),
            node("i", OpKind::Invert, &["c"]),
        ],
        "i",
        4,
    );
    let p = img.pixels()[0];
    assert!((p[0] - 0.75).abs() < 1e-5);
    assert!((p[1] - 0.5).abs() < 1e-5);
    assert!((p[2] - 0.25).abs() < 1e-5);
}

#[test]
fn mix_add_combines_inputs() {
    let img = run(
        vec![
            constant("a", [0.4, 0.4, 0.4, 1.0]),
            constant("b", [0.5, 0.5, 0.5, 1.0]),
            node(
                "m",
                OpKind::Mix {
                    mode: BlendMode::Add,
                    factor: 1.0,
                },
                &["a", "b"],
            ),
        ],
        "m",
        4,
    );
    assert!((img.pixels()[0][0] - 0.9).abs() < 1e-5, "0.4 + 0.5");
}

#[test]
fn mix_multiply_combines_inputs() {
    let img = run(
        vec![
            constant("a", [0.4, 0.4, 0.4, 1.0]),
            constant("b", [0.5, 0.5, 0.5, 1.0]),
            node(
                "m",
                OpKind::Mix {
                    mode: BlendMode::Multiply,
                    factor: 1.0,
                },
                &["a", "b"],
            ),
        ],
        "m",
        4,
    );
    assert!((img.pixels()[0][0] - 0.2).abs() < 1e-5, "0.4 * 0.5");
}
