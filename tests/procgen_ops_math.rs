//! Per-op correctness: converter/math, vector/normal, and filter ops (#270). The
//! generator + color ops are covered in `procgen_ops.rs`. Each op is exercised through
//! the public runner (`evaluate`) so the test pins observable pixel behaviour.

use rusty::procgen::recipe::{GradientKind, Node, OpKind, TextureRecipe};
use rusty::procgen::{evaluate, Image};

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
fn clamp_bounds_each_channel() {
    let img = run(
        vec![
            constant("c", [2.0, -1.0, 0.5, 1.0]),
            node("cl", OpKind::Clamp { min: 0.0, max: 1.0 }, &["c"]),
        ],
        "cl",
        2,
    );
    let p = img.pixels()[0];
    assert_eq!([p[0], p[1], p[2]], [1.0, 0.0, 0.5]);
}

#[test]
fn map_range_remaps_channels() {
    let img = run(
        vec![
            constant("c", [0.5, 0.5, 0.5, 1.0]),
            node(
                "mr",
                OpKind::MapRange {
                    from_min: 0.0,
                    from_max: 1.0,
                    to_min: 0.0,
                    to_max: 10.0,
                },
                &["c"],
            ),
        ],
        "mr",
        2,
    );
    assert!((img.pixels()[0][0] - 5.0).abs() < 1e-5);
}

#[test]
fn rgb_to_bw_uses_rec709_luminance() {
    // Pure green -> 0.7152 in all channels.
    let img = run(
        vec![
            constant("c", [0.0, 1.0, 0.0, 1.0]),
            node("bw", OpKind::RgbToBw, &["c"]),
        ],
        "bw",
        2,
    );
    let p = img.pixels()[0];
    assert!((p[0] - 0.7152).abs() < 1e-4);
    assert_eq!(p[0], p[1]);
    assert_eq!(p[1], p[2]);
}

#[test]
fn bump_to_normal_tilts_toward_a_rising_height() {
    // A left->right height ramp: the normal should tilt in -X (red < 0.5) and stay
    // mostly +Z (blue ~ 1).
    let img = run(
        vec![
            node(
                "g",
                OpKind::Gradient {
                    kind: GradientKind::Linear,
                },
                &[],
            ),
            node("n", OpKind::BumpToNormal { strength: 4.0 }, &["g"]),
        ],
        "n",
        32,
    );
    // Sample an interior pixel away from the wrap seam.
    let p = img.get_wrapped(16, 16);
    assert!(
        p[0] < 0.5,
        "normal.x should tilt negative on a rising ramp, got {}",
        p[0]
    );
    assert!(p[2] > 0.5, "normal.z should remain positive, got {}", p[2]);
}

#[test]
fn blur_spreads_a_hard_edge_into_a_gradient() {
    // Blurring a checker softens its hard tile boundary: the pixel straddling the
    // edge becomes an intermediate gray, proving the box window spreads energy.
    let img = run(
        vec![
            node(
                "k",
                OpKind::Checker {
                    tiles: 2,
                    color_a: [0.0, 0.0, 0.0, 1.0],
                    color_b: [1.0, 1.0, 1.0, 1.0],
                },
                &[],
            ),
            node("b", OpKind::Blur { radius: 1 }, &["k"]),
        ],
        "b",
        8,
    );
    let edge = img.get_wrapped(3, 0)[0];
    assert!(
        edge > 0.0 && edge < 1.0,
        "blur softens the edge, got {edge}"
    );
}
