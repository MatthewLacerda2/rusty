//! #407's hard-surface ops — `shape`, `tile` scatter, brick `random` / `bevel` —
//! keep the two procgen promises (seamless, resolution-independent) and bake
//! byte-identically for a seed. The generators' seams are swept in `procgen_seams`.

use rusty::procgen::recipe::{BrickOutput, Node, OpKind, ShapeKind, TextureRecipe};
use rusty::procgen::{evaluate, Image};

use crate::procgen_resolution::assert_resolution_independent;
use crate::procgen_seams::{assert_seamless, bake, node};

fn shape(kind: ShapeKind, softness: f32) -> Node {
    let op = OpKind::Shape {
        kind,
        size: [0.7, 0.5],
        roundness: 0.4,
        softness,
    };
    node("s", op, &[])
}

/// Rivets: a soft circle scattered over a jittered grid.
fn rivets(jitter: f32, rotation_jitter: f32, scale_jitter: f32) -> Vec<Node> {
    let tile = OpKind::Tile {
        count: 5.0,
        jitter,
        rotation_jitter,
        scale_jitter,
    };
    vec![shape(ShapeKind::Circle, 0.1), node("t", tile, &["s"])]
}

fn brick(output: BrickOutput) -> Vec<Node> {
    let op = OpKind::Brick {
        rows: 6.0,
        cols: 3.0,
        mortar: 0.08,
        output,
    };
    vec![node("b", op, &[])]
}

fn bake_seeded(nodes: &[Node], seed: u64) -> Image {
    let recipe = TextureRecipe::new(64, nodes.to_vec()).with_seed(seed);
    evaluate(&recipe).expect("recipe evaluates")
}

/// #407's brick outputs and shapes, at sizes that clip at the tile edge.
fn hard_surface() -> Vec<(String, OpKind)> {
    let mut ops = Vec::new();
    for output in [BrickOutput::Mask, BrickOutput::Random, BrickOutput::Bevel] {
        let op = OpKind::Brick {
            rows: 5.0,
            cols: 2.6,
            mortar: 0.1,
            output,
        };
        ops.push((format!("brick {output:?}"), op));
    }
    for kind in [ShapeKind::Circle, ShapeKind::RoundedRect, ShapeKind::Line] {
        let op = OpKind::Shape {
            kind,
            size: [1.3, 0.4],
            roundness: 0.5,
            softness: 0.1,
        };
        ops.push((format!("shape {kind:?}"), op));
    }
    ops
}

#[test]
fn hard_surface_generators_bake_without_a_seam() {
    for (label, op) in hard_surface() {
        assert_seamless(&label, &bake(vec![node("g", op, &[])]));
    }
}

#[test]
fn tile_scatter_stays_seamless_at_every_jitter() {
    for (j, r, s) in [
        (0.0, 0.0, 0.0),
        (1.0, 0.0, 0.0),
        (0.6, 0.3, 0.5),
        (1.0, 1.0, 1.0),
    ] {
        assert_seamless(&format!("tile {j} {r} {s}"), &bake(rivets(j, r, s)));
    }
}

#[test]
fn tile_of_a_rect_panel_has_no_gaps_at_the_cell_borders() {
    // A full-cell rect, untouched by jitter, tiles into one flat white sheet.
    let op = OpKind::Shape {
        kind: ShapeKind::Rect,
        size: [1.0, 1.0],
        roundness: 0.0,
        softness: 0.0,
    };
    let tile = OpKind::Tile {
        count: 3.0,
        jitter: 0.0,
        rotation_jitter: 0.0,
        scale_jitter: 0.0,
    };
    let img = bake(vec![node("s", op, &[]), node("t", tile, &["s"])]);
    assert!(img.pixels().iter().all(|p| p[0] == 1.0));
}

#[test]
fn hard_surface_ops_are_resolution_independent() {
    for kind in [ShapeKind::Circle, ShapeKind::RoundedRect, ShapeKind::Line] {
        assert_resolution_independent(&format!("shape {kind:?}"), &[shape(kind, 0.05)], 0.02);
    }
    assert_resolution_independent("tile", &rivets(0.8, 0.5, 0.5), 0.03);
    for output in [BrickOutput::Random, BrickOutput::Bevel] {
        assert_resolution_independent(&format!("brick {output:?}"), &brick(output), 0.03);
    }
}

#[test]
fn seeded_hard_surface_ops_bake_byte_identically_per_seed() {
    let recipes = [
        rivets(0.7, 0.4, 0.6),
        brick(BrickOutput::Random),
        brick(BrickOutput::Bevel),
        vec![shape(ShapeKind::RoundedRect, 0.1)],
    ];
    for (i, nodes) in recipes.iter().enumerate() {
        let (a, b) = (bake_seeded(nodes, 11), bake_seeded(nodes, 11));
        assert_eq!(
            a.pixels(),
            b.pixels(),
            "recipe {i}: same seed, different bake"
        );
        // The first two draw on the seed; bevel and shape must not.
        let other = bake_seeded(nodes, 12);
        assert_eq!(a.pixels() != other.pixels(), i < 2, "recipe {i}: seed use");
    }
}
