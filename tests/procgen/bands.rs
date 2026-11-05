//! The row-band split is invisible (#1490): each recipe below touches ops that paint
//! by rows, and bakes to the same bytes at 1 band and at N, with an uneven split.
//!
//! One row per op or family over a shared prefix (#1837), so a new op adds a row and
//! two branches land on different lines.

use rusty::procgen::{evaluate, with_bands, TextureRecipe};

/// What the rows read: a height, a noise, its blur, a stamp and a bevel.
const PREFIX: &str = r#"
    { "id": "h", "op": "brick", "rows": 4, "cols": 2, "mortar": 0.06 },
    { "id": "chips", "op": "noise", "kind": "fbm", "scale": 8, "octaves": 4 },
    { "id": "soft", "op": "blur", "radius": 0.07, "inputs": ["chips"] },
    { "id": "dots", "op": "shape", "kind": "circle", "size": [0.5, 0.5] },
    { "id": "bev", "op": "distance", "radius": 0.1, "inputs": ["h"] },
"#;

/// One row per line: its name, the node it outputs, and its nodes. A raw string, so
/// rustfmt leaves each row on its one line.
const ROWS: &str = r#"
// sorted: one row per op or family, so two branches adding ops land on different lines
ambient_occlusion ao { "id": "ao", "op": "ambient_occlusion", "radius": 0.05, "quality": 4, "inputs": ["h"] }
anisotropic_noise fib { "id": "fib", "op": "anisotropic_noise", "scale": 24, "anisotropy": 6, "angle": 0.1, "seed": 2 }
auto_levels lev { "id": "lev", "op": "auto_levels", "inputs": ["chips"] }
bump_and_normal_blend nb { "id": "bn", "op": "bump_to_normal", "strength": 0.3, "inputs": ["bev"] }, { "id": "n", "op": "bump_to_normal", "strength": 0.4, "inputs": ["soft"] }, { "id": "nb", "op": "normal_blend", "inputs": ["bn", "n"] }
cavity_and_channels hsv { "id": "cav", "op": "cavity", "radius": 0.02, "inputs": ["h"] }, { "id": "rgb", "op": "combine_rgb", "inputs": ["cav", "chips", "soft"] }, { "id": "hsv", "op": "hue_sat_value", "hue": 0.1, "sat": 0.8, "value": 1.1, "inputs": ["rgb"] }
curvature_and_mix worn { "id": "paint", "op": "constant", "color": [0.20, 0.32, 0.18, 1] }, { "id": "metal", "op": "constant", "color": [0.55, 0.56, 0.58, 1] }, { "id": "curv", "op": "curvature", "radius": 0.01, "strength": 3, "inputs": ["h"] }, { "id": "edge", "op": "map_range", "from_min": 0.6, "from_max": 0.7, "to_min": 0, "to_max": 1, "inputs": ["curv"] }, { "id": "wear", "op": "mix", "mode": "multiply", "inputs": ["edge", "chips"] }, { "id": "worn", "op": "mix", "mode": "mix", "inputs": ["paint", "metal", "wear"] }
directional_blur db { "id": "db", "op": "directional_blur", "length": 0.1, "angle": 0.125, "inputs": ["chips"] }
directional_warp dw { "id": "dw", "op": "directional_warp", "strength": 0.05, "angle": 0.3, "inputs": ["chips", "h"] }
edge_detect ed { "id": "ed", "op": "edge_detect", "width": 0.03, "roundness": 0.5, "inputs": ["h"] }
flood flood { "id": "ff", "op": "flood_fill", "threshold": 0.55, "inputs": ["chips"] }, { "id": "fr", "op": "flood_random", "seed": 3, "inputs": ["ff"] }, { "id": "fg", "op": "flood_gradient", "angle": 0.1, "angle_jitter": 0.2, "seed": 1, "inputs": ["ff"] }, { "id": "fb", "op": "flood_bbox", "output": "center_x", "inputs": ["ff"] }, { "id": "flood", "op": "combine_rgb", "inputs": ["fr", "fg", "fb"] }
highpass_and_sharpen sh { "id": "hp", "op": "highpass", "radius": 0.05, "inputs": ["chips"] }, { "id": "sh", "op": "sharpen", "intensity": 1.5, "radius": 0.02, "inputs": ["hp"] }
histogram_and_levels lv { "id": "hs", "op": "histogram_scan", "position": 0.4, "contrast": 0.6, "inputs": ["chips"] }, { "id": "lv", "op": "levels", "in_low": 0.1, "gamma": 1.4, "inputs": ["hs"] }
math top { "id": "top", "op": "math", "func": "max", "value": 0, "inputs": ["soft", "chips"] }
mix_modes div { "id": "ov", "op": "mix", "mode": "overlay", "inputs": ["h", "chips"] }, { "id": "li", "op": "mix", "mode": "lighten", "inputs": ["ov", "soft"] }, { "id": "sc", "op": "mix", "mode": "screen", "factor": 0.4, "inputs": ["li", "bev"] }, { "id": "ad", "op": "mix", "mode": "add", "factor": 0.4, "inputs": ["sc", "chips"] }, { "id": "div", "op": "mix", "mode": "divide", "factor": 0.3, "inputs": ["ad", "soft"] }
noise_distorted dn { "id": "dn", "op": "noise", "kind": "fbm", "scale": 4, "octaves": 3, "distortion": 1.5 }
shape_polygon pg { "id": "pg", "op": "shape", "kind": "polygon", "size": [0.6, 0.5], "sides": 7, "profile": "hemisphere" }
shape_star st { "id": "st", "op": "shape", "kind": "star", "size": [1.2, 1.2], "points": 5, "profile": "cone" }
slope_blur sb { "id": "sb", "op": "slope_blur", "intensity": 0.01, "mode": "max", "inputs": ["chips", "bev"] }
tile_add piled { "id": "piled", "op": "tile", "count": 7, "jitter": 1, "blend": "add", "inputs": ["dots"] }
tile_over_masked leaves { "id": "leaves", "op": "tile", "count": 6, "jitter": 0.8, "rotation_jitter": 1, "scale_jitter": 0.5, "value_jitter": 0.6, "probability": 0.8, "output": "random", "blend": "over", "inputs": ["dots", "chips", "soft"] }
tile_warped_and_mapped turned { "id": "rivets", "op": "tile", "count": 5, "jitter": 0.3, "inputs": ["dots"] }, { "id": "bent", "op": "warp", "strength": 0.1, "inputs": ["rivets", "soft"] }, { "id": "turned", "op": "mapping", "scale": [2, 1], "rotation": 0.1, "translation": [0.1, 0], "inputs": ["bent"] }
voronoi_smooth_f1 sf { "id": "sf", "op": "voronoi", "scale": 6, "output": "smooth_f1", "smoothness": 0.6 }
wave_distorted saw { "id": "saw", "op": "wave", "kind": "bands", "frequency": 5, "profile": "saw", "distortion": 4, "detail": 3 }
"#;

/// `row`'s name and recipe.
fn recipe(row: &str) -> (&str, TextureRecipe) {
    let mut parts = row.splitn(3, ' ');
    let (name, output, nodes) = (
        parts.next().expect("a name"),
        parts.next().expect("an output"),
        parts.next().expect("its nodes"),
    );
    let json = format!(
        r#"{{ "resolution": 100, "seed": 4, "nodes": [{PREFIX}{nodes}], "output": "{output}" }}"#
    );
    (name, serde_json::from_str(&json).expect("recipe parses"))
}

/// The output pixels' bit patterns, evaluated under `bands` bands.
fn bits(recipe: &TextureRecipe, bands: usize) -> Vec<u32> {
    let img = with_bands(bands, || evaluate(recipe)).expect("recipe evaluates");
    img.pixels().iter().flatten().map(|c| c.to_bits()).collect()
}

#[test]
fn every_band_count_bakes_the_same_bytes() {
    let rows = ROWS
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with("//"));
    for row in rows {
        let (name, recipe) = recipe(row);
        let serial = bits(&recipe, 1);
        for bands in [2, 3, 7] {
            assert!(
                bits(&recipe, bands) == serial,
                "{name}: {bands} bands differ from the serial bake"
            );
        }
    }
}
