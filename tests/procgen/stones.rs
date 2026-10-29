//! #1492's ops in a graph: the *cobblestone* example (docs/api/Texture.md) bevels
//! Chebyshev Voronoi cells with `distance` and layers a detail normal over them.
//! The bevel and the blend keep the two procgen promises: seamless and
//! resolution-independent.

use rusty::procgen::recipe::{Node, TextureRecipe};
use rusty::procgen::{evaluate, Image};

use crate::procgen::resolution::assert_resolution_independent;
use crate::procgen::seams::assert_seamless;

/// The cobblestone normal map, as the docs write it, plus a grain blended over it.
const RECIPE: &str = r##"{
  "resolution": 64, "seed": 2,
  "nodes": [
    { "id": "cells", "op": "voronoi", "scale": 6, "output": "edges", "metric": "chebyshev" },
    { "id": "stones", "op": "map_range", "from_min": 0.06, "from_max": 0.1,
      "to_min": 0, "to_max": 1, "inputs": ["cells"] },
    { "id": "bevel", "op": "distance", "radius": 0.04, "inputs": ["stones"] },
    { "id": "round", "op": "math", "func": "sqrt", "inputs": ["bevel"] },
    { "id": "n", "op": "bump_to_normal", "strength": 0.5, "inputs": ["round"] },
    { "id": "grain", "op": "noise", "kind": "fbm", "scale": 24, "octaves": 3 },
    { "id": "gn", "op": "bump_to_normal", "strength": 0.1, "inputs": ["grain"] },
    { "id": "out", "op": "normal_blend", "strength": 0.5, "inputs": ["n", "gn"] }
  ]
}"##;

fn nodes() -> Vec<Node> {
    let recipe: TextureRecipe = serde_json::from_str(RECIPE).expect("recipe parses");
    recipe.nodes
}

fn bake(resolution: u32) -> Image {
    evaluate(&TextureRecipe::new(resolution, nodes()).with_seed(2)).expect("recipe evaluates")
}

#[test]
fn the_cobblestone_bakes_rounded_stones_with_flat_gaps() {
    let img = bake(64);
    assert_seamless("cobblestone", &img);
    // Stones tilt towards their rims; the mortar between them stays flat.
    let tilted = img.pixels().iter().filter(|p| p[2] < 0.97).count();
    let flat = img.pixels().iter().filter(|p| p[2] > 0.99).count();
    let n = img.pixels().len();
    assert!(
        tilted > n / 10 && flat > n / 20,
        "{tilted} tilted, {flat} flat of {n}"
    );
}

#[test]
fn the_cobblestone_bevel_is_in_tile_units() {
    let bevel: Vec<Node> = nodes().into_iter().take(3).collect();
    assert_resolution_independent("distance", &bevel, 0.03);
}
