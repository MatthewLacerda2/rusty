//! The row-band split is invisible (#1490): a recipe touching every op that paints
//! by rows bakes to the same bytes at 1 band and at N, with an uneven split.

use rusty::procgen::{evaluate, with_bands, TextureRecipe};

/// The *worn, grimy painted metal* example (docs/api/Texture.md) plus a blur,
/// warp, tile, mapping, bump, a distance bevel, a normal blend, the channel ops,
/// two-image math and auto levels, so every banded path runs.
const RECIPE: &str = r#"{
  "resolution": 100, "seed": 4,
  "nodes": [
    { "id": "h", "op": "brick", "rows": 4, "cols": 2, "mortar": 0.06 },
    { "id": "paint", "op": "constant", "color": [0.20, 0.32, 0.18, 1] },
    { "id": "metal", "op": "constant", "color": [0.55, 0.56, 0.58, 1] },
    { "id": "curv", "op": "curvature", "radius": 0.01, "strength": 3, "inputs": ["h"] },
    { "id": "edge", "op": "map_range", "from_min": 0.6, "from_max": 0.7,
      "to_min": 0, "to_max": 1, "inputs": ["curv"] },
    { "id": "chips", "op": "noise", "kind": "fbm", "scale": 8, "octaves": 4 },
    { "id": "wear", "op": "mix", "mode": "multiply", "inputs": ["edge", "chips"] },
    { "id": "worn", "op": "mix", "mode": "mix", "inputs": ["paint", "metal", "wear"] },
    { "id": "cav", "op": "cavity", "radius": 0.02, "inputs": ["h"] },
    { "id": "soft", "op": "blur", "radius": 0.07, "inputs": ["chips"] },
    { "id": "dots", "op": "shape", "kind": "circle", "size": [0.5, 0.5] },
    { "id": "rivets", "op": "tile", "count": 5, "jitter": 0.3, "inputs": ["dots"] },
    { "id": "bent", "op": "warp", "strength": 0.1, "inputs": ["rivets", "soft"] },
    { "id": "turned", "op": "mapping", "scale": [2, 1], "rotation": 0.1, "translation": [0.1, 0], "inputs": ["bent"] },
    { "id": "n", "op": "bump_to_normal", "strength": 0.4, "inputs": ["turned"] },
    { "id": "bev", "op": "distance", "radius": 0.1, "inputs": ["h"] },
    { "id": "bn", "op": "bump_to_normal", "strength": 0.3, "inputs": ["bev"] },
    { "id": "nb", "op": "normal_blend", "inputs": ["bn", "n"] },
    { "id": "rgb", "op": "combine_rgb", "inputs": ["cav", "nb", "soft"] },
    { "id": "hsv", "op": "hue_sat_value", "hue": 0.1, "sat": 0.8, "value": 1.1, "inputs": ["rgb"] },
    { "id": "lit", "op": "mix", "mode": "overlay", "inputs": ["worn", "hsv"] },
    { "id": "top", "op": "math", "func": "max", "value": 0, "inputs": ["lit", "chips"] },
    { "id": "lev", "op": "auto_levels", "inputs": ["top"] },
    { "id": "lt", "op": "mix", "mode": "lighten", "inputs": ["lev", "cav"] },
    { "id": "out", "op": "mix", "mode": "divide", "factor": 0.3, "inputs": ["lt", "soft"] }
  ],
  "output": "out"
}"#;

/// The output pixels' bit patterns, evaluated under `bands` bands.
fn bits(bands: usize) -> Vec<u32> {
    let recipe: TextureRecipe = serde_json::from_str(RECIPE).expect("recipe parses");
    let img = with_bands(bands, || evaluate(&recipe)).expect("recipe evaluates");
    img.pixels().iter().flatten().map(|c| c.to_bits()).collect()
}

#[test]
fn every_band_count_bakes_the_same_bytes() {
    let serial = bits(1);
    for bands in [2, 3, 7] {
        assert!(
            bits(bands) == serial,
            "{bands} bands differ from the serial bake"
        );
    }
}
