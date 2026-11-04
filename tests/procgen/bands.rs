//! The row-band split is invisible (#1490): a recipe touching every op that paints
//! by rows bakes to the same bytes at 1 band and at N, with an uneven split.

use rusty::procgen::{evaluate, with_bands, TextureRecipe};

/// The *worn, grimy painted metal* example (docs/api/Texture.md) plus a blur,
/// warp, tile, mapping, bump, a distance bevel, a normal blend, the channel ops,
/// two-image math, auto levels and the #1743 filters (occlusion, edges, highpass,
/// sharpen, histogram scan, levels) and #1740's generators (fibres, a distorted saw
/// wave, a distorted noise, smooth F1, a star and a polygon with heights), so every
/// banded path runs.
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
    { "id": "ao", "op": "ambient_occlusion", "radius": 0.05, "quality": 4, "inputs": ["h"] },
    { "id": "ed", "op": "edge_detect", "width": 0.03, "roundness": 0.5, "inputs": ["h"] },
    { "id": "hp", "op": "highpass", "radius": 0.05, "inputs": ["chips"] },
    { "id": "sh", "op": "sharpen", "intensity": 1.5, "radius": 0.02, "inputs": ["hp"] },
    { "id": "hs", "op": "histogram_scan", "position": 0.4, "contrast": 0.6, "inputs": ["sh"] },
    { "id": "lv", "op": "levels", "in_low": 0.1, "gamma": 1.4, "inputs": ["hs"] },
    { "id": "more", "op": "combine_rgb", "inputs": ["ao", "ed", "lv"] },
    { "id": "lt2", "op": "mix", "mode": "multiply", "factor": 0.5, "inputs": ["lt", "more"] },
    { "id": "fib", "op": "anisotropic_noise", "scale": 24, "anisotropy": 6, "angle": 0.1, "seed": 2 },
    { "id": "saw", "op": "wave", "kind": "bands", "frequency": 5, "profile": "saw", "distortion": 4, "detail": 3 },
    { "id": "dn", "op": "noise", "kind": "fbm", "scale": 4, "octaves": 3, "distortion": 1.5 },
    { "id": "sf", "op": "voronoi", "scale": 6, "output": "smooth_f1", "smoothness": 0.6 },
    { "id": "st", "op": "shape", "kind": "star", "size": [1.2, 1.2], "points": 5, "profile": "cone" },
    { "id": "pg", "op": "shape", "kind": "polygon", "size": [0.6, 0.5], "sides": 7, "profile": "hemisphere" },
    { "id": "gen", "op": "combine_rgb", "inputs": ["fib", "saw", "dn"] },
    { "id": "gen2", "op": "combine_rgb", "inputs": ["sf", "st", "pg"] },
    { "id": "gens", "op": "mix", "mode": "screen", "inputs": ["gen", "gen2"] },
    { "id": "lt3", "op": "mix", "mode": "overlay", "factor": 0.5, "inputs": ["lt2", "gens"] },
    { "id": "out", "op": "mix", "mode": "divide", "factor": 0.3, "inputs": ["lt3", "soft"] }
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
