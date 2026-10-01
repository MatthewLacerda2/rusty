//! #405's mask-driven `mix` and #408's height-derived masks (`cavity`,
//! `curvature`): what each one picks out, and byte-identical bakes per seed. Their
//! seams and resolution independence are swept in `procgen_seams` / `_resolution`.

use rusty::procgen::recipe::{BlendMode, Node, NoiseKind, OpKind, TextureRecipe};
use rusty::procgen::{evaluate, Image};

use crate::procgen_seams::{bake, node};

const RES: usize = 64;

fn constant(id: &str, v: f32) -> Node {
    node(
        id,
        OpKind::Constant {
            color: [v, v, v, 1.0],
        },
        &[],
    )
}

fn mix(mode: BlendMode, inputs: &[&str]) -> Node {
    node("m", OpKind::Mix { mode, factor: 1.0 }, inputs)
}

/// A brick wall's height: bricks at 1, mortar at 0.
fn bricks() -> Node {
    let op = OpKind::Brick {
        rows: 4.0,
        cols: 2.0,
        mortar: 0.1,
        output: rusty::procgen::recipe::BrickOutput::Mask,
    };
    node("h", op, &[])
}

#[test]
fn a_mask_picks_a_where_black_and_the_blend_where_white() {
    let linear = OpKind::Gradient {
        kind: rusty::procgen::recipe::GradientKind::Linear,
    };
    let step = OpKind::MapRange {
        from_min: 0.5,
        from_max: 0.501,
        to_min: 0.0,
        to_max: 1.0,
    };
    let img = bake(vec![
        constant("a", 0.2),
        constant("b", 0.6),
        node("g", linear, &[]),
        node("s", step, &["g"]),
        node("mask", OpKind::Clamp { min: 0.0, max: 1.0 }, &["s"]), // black | white
        mix(BlendMode::Add, &["a", "b", "mask"]),
    ]);
    let row = &img.pixels()[RES * 10..RES * 11];
    assert!(
        row[..RES / 2].iter().all(|p| (p[0] - 0.2).abs() < 1e-5),
        "left: a"
    );
    assert!(
        row[RES / 2..].iter().all(|p| (p[0] - 0.8).abs() < 1e-5),
        "right: a + b"
    );
}

#[test]
fn factor_defaults_to_one() {
    let json = r#"{ "id": "m", "op": "mix", "mode": "screen" }"#;
    let parsed: Node = serde_json::from_str(json).expect("factor is optional");
    assert!(matches!(parsed.op, OpKind::Mix { factor, .. } if factor == 1.0));
}

/// Mean red of `img` over mortar pixels and over brick pixels of `height`.
fn mortar_and_brick_means(img: &Image, height: &Image) -> (f32, f32) {
    let (mut m, mut b) = ((0.0, 0), (0.0, 0));
    for (p, h) in img.pixels().iter().zip(height.pixels()) {
        let acc = if h[0] < 0.5 { &mut m } else { &mut b };
        acc.0 += p[0];
        acc.1 += 1;
    }
    (m.0 / m.1 as f32, b.0 / b.1 as f32)
}

#[test]
fn cavity_darkens_crevices_and_curvature_splits_edges_from_grooves() {
    let height = bake(vec![bricks()]);
    let cavity = bake(vec![
        bricks(),
        node("c", OpKind::Cavity { radius: 0.05 }, &["h"]),
    ]);
    let (mortar, brick) = mortar_and_brick_means(&cavity, &height);
    assert!(
        mortar < 0.7 && brick > 0.95,
        "cavity mortar {mortar}, brick {brick}"
    );

    let op = OpKind::Curvature {
        radius: 0.03,
        strength: 1.0,
    };
    let curv = bake(vec![bricks(), node("c", op, &["h"])]);
    let (mortar, brick) = mortar_and_brick_means(&curv, &height);
    assert!(
        mortar < 0.5 && brick > 0.5,
        "curvature mortar {mortar}, brick {brick}"
    );
}

#[test]
fn weathering_bakes_byte_identically_per_seed() {
    let noise = OpKind::Noise {
        kind: NoiseKind::Fbm,
        scale: 4.0,
        octaves: 3,
        lacunarity: 2.0,
        gain: 0.5,
    };
    let nodes = vec![
        node("n", noise, &[]),
        node("cav", OpKind::Cavity { radius: 0.03 }, &["n"]),
        node(
            "cur",
            OpKind::Curvature {
                radius: 0.02,
                strength: 2.0,
            },
            &["n"],
        ),
        mix(BlendMode::Multiply, &["cav", "n", "cur"]),
    ];
    let at = |seed| {
        let recipe = TextureRecipe {
            resolution: 64,
            seed,
            nodes: nodes.clone(),
            output: None,
            outputs: Default::default(),
        };
        evaluate(&recipe).expect("recipe evaluates")
    };
    assert_eq!(at(5).pixels(), at(5).pixels(), "same seed, different bake");
    assert_ne!(at(5).pixels(), at(6).pixels(), "seed ignored");
}
