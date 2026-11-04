//! #1742's Tile Sampler params keep `tile`'s promise to tile: each one, with
//! stamps pushed across the tile's edge, bakes without a seam.

use rusty::procgen::{evaluate, TextureRecipe};

use crate::procgen::seams::assert_seamless;

/// A soft disc baked at `res`², stamped over a `count`-cell grid at full jitter, so stamps cross
/// the tile's edge, plus `extra` params on the `tile` node and its `inputs`.
fn bake(res: u32, count: u32, extra: &str, inputs: &str) -> rusty::procgen::Image {
    let json = format!(
        r#"{{ "resolution": {res}, "seed": 7, "nodes": [
      {{ "id": "s", "op": "shape", "kind": "circle", "size": [0.9, 0.9], "softness": 0.3 }},
      {{ "id": "m", "op": "noise", "kind": "fbm", "scale": 3, "octaves": 2 }},
      {{ "id": "big", "op": "constant", "color": [1.6, 0, 0, 1] }},
      {{ "id": "w", "op": "constant", "color": [1, 1, 1, 1] }},
      {{ "id": "t", "op": "tile", "count": {count}, "jitter": 1, "rotation_jitter": 1,
        "scale_jitter": 0.5{extra}, "inputs": [{inputs}] }}
    ], "output": "t" }}"#
    );
    let recipe: TextureRecipe = serde_json::from_str(&json).expect("recipe parses");
    evaluate(&recipe).expect("recipe evaluates")
}

#[test]
fn every_tile_sampler_param_bakes_without_a_seam() {
    let cases = [
        ("output random", r#", "output": "random""#, r#""s""#),
        ("output index", r#", "output": "index""#, r#""s""#),
        (
            "probability",
            r#", "probability": 0.5, "value_jitter": 1"#,
            r#""s""#,
        ),
        ("mask", "", r#""s", "m""#),
        ("size map 1.6", "", r#""s", "m", "big""#),
        ("add", r#", "blend": "add""#, r#""s", "m", "big""#),
        (
            "over masked",
            r#", "blend": "over", "probability": 0.7, "value_jitter": 1"#,
            r#""s", "m", "big""#,
        ),
    ];
    for (label, extra, inputs) in cases {
        assert_seamless(label, &bake(64, 5, extra, inputs));
    }
}

#[test]
fn over_composites_in_the_same_order_on_both_sides_of_the_edge() {
    // Order is part of `over`'s result once stamps differ in grey (with one grey
    // it is a screen, the same either way). Smooth stamps at 256² keep every
    // interior step small, so stamps stacked in another order across the edge
    // would show; at one cell, the stamp meets its own copies across the wrap.
    for count in [1, 2, 5] {
        let extra = r#", "blend": "over", "output": "random""#;
        let img = bake(256, count, extra, r#""s", "w", "big""#);
        assert_seamless(&format!("over at count {count}"), &img);
    }
}
