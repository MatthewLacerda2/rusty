//! #1494's acceptance: a linked param varies across the map. A warp whose strength
//! follows a half-black / half-white mask is the input on one half and the full
//! warp on the other; a tile whose jitter follows a ramp is ordered at its dark end
//! and still tiles.

use rusty::procgen::recipe::Node;
use rusty::procgen::{evaluate, Image, TextureRecipe};
use serde_json::{json, Value};

use super::seams::seam_and_interior;

/// A node parsed from its JSON form, as a recipe file writes it.
fn node(doc: Value) -> Node {
    serde_json::from_value(doc).unwrap()
}

fn bake(nodes: Vec<Value>, output: &str, size: [u32; 2]) -> Image {
    let nodes = nodes.into_iter().map(node).collect();
    let recipe = TextureRecipe::new(0, nodes)
        .with_size(size)
        .with_seed(11)
        .with_output(output);
    evaluate(&recipe).unwrap()
}

/// A noise to warp, a displacement, and a mask black on the left half of the tile
/// and white on the right.
fn warp_inputs() -> Vec<Value> {
    vec![
        json!({ "id": "a", "op": "noise", "kind": "fbm", "scale": 6, "octaves": 3 }),
        json!({ "id": "d", "op": "voronoi", "scale": 3, "output": "cells" }),
        json!({ "id": "g", "op": "gradient", "kind": "linear" }),
        json!({ "id": "mask", "op": "color_ramp", "interpolation": "constant", "inputs": ["g"],
                "stops": [{ "pos": 0.0, "color": [0, 0, 0, 1] }, { "pos": 0.5, "color": [1, 1, 1, 1] }] }),
    ]
}

#[test]
fn a_warp_linked_to_a_half_mask_is_the_input_then_the_full_warp() {
    let size = [64, 32];
    let warp = |strength: Value| {
        let mut nodes = warp_inputs();
        nodes.push(json!({ "id": "w", "op": "warp", "strength": strength, "inputs": ["a", "d"] }));
        bake(nodes, "w", size)
    };
    let linked = warp(json!({ "node": "mask", "min": 0.0, "max": 0.2 }));
    let (input, full) = (bake(warp_inputs(), "a", size), warp(json!(0.2)));
    assert_ne!(input, full, "the full warp moves the input");
    for y in 0..size[1] as i64 {
        for x in 0..size[0] as i64 {
            let want = if x < 32 { &input } else { &full };
            assert_eq!(
                linked.get_wrapped(x, y),
                want.get_wrapped(x, y),
                "({x}, {y})"
            );
        }
    }
}

/// Soft cones on a 4×4 grid, their jitter linked to `jitter`. A full jitter pushes
/// a cone across its cell's border, and the last column's across the wrap.
fn dots(jitter: Value) -> Image {
    let nodes = vec![
        json!({ "id": "dot", "op": "shape", "kind": "circle", "size": [0.8, 0.8], "softness": 0.4 }),
        json!({ "id": "ramp", "op": "gradient", "kind": "linear" }),
        json!({ "id": "t", "op": "tile", "count": 4, "jitter": jitter, "scale_jitter": jitter,
                "seed": 4, "inputs": ["dot"] }),
    ];
    bake(nodes, "t", [64, 64])
}

/// Mean absolute red difference between `a` and `b` over columns `xs`.
fn diff(a: &Image, b: &Image, xs: std::ops::Range<i64>) -> f32 {
    let mut sum = 0.0;
    for y in 0..64 {
        for x in xs.clone() {
            sum += (a.get_wrapped(x, y)[0] - b.get_wrapped(x, y)[0]).abs();
        }
    }
    sum / (64 * (xs.end - xs.start)) as f32
}

#[test]
fn a_tile_jitter_linked_to_a_ramp_is_ordered_at_its_dark_end_and_tiles() {
    let ordered = dots(json!(0.0));
    let ramped = dots(json!({ "node": "ramp" }));
    let chaotic = dots(json!(1.0));
    // The first column of cells (jitter 0.125) sits nearly on the grid; the last
    // (0.875) is nearly as scattered as full jitter.
    let (dark, bright) = (
        diff(&ramped, &ordered, 0..16),
        diff(&ramped, &ordered, 48..64),
    );
    assert!(
        dark * 3.0 < bright,
        "dark end {dark} vs bright end {bright}"
    );
    assert!(
        diff(&ramped, &chaotic, 48..64) < bright,
        "the bright end is near full jitter"
    );
    // One value per cell, read at its wrapped centre: no seam across either wrap,
    // though the ramp itself jumps from white to black there. A cone cut at the
    // wrap would jump far more than the soft cones' slopes do inside.
    for horizontal in [true, false] {
        let (seam, interior) = seam_and_interior(&ramped, horizontal);
        assert!(
            seam <= interior,
            "seam {seam} > interior {interior} ({horizontal})"
        );
    }
}
