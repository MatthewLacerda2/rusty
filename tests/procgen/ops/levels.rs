//! Image-by-image math, the min/max/divide blends, `auto_levels` and sRGB hex
//! colours (#1487).

use rusty::procgen::recipe::TextureRecipe;
use rusty::procgen::{evaluate, Image};

/// Bake a recipe given as JSON, reading its `output` (or last) node.
fn bake(json: &str) -> Image {
    let recipe: TextureRecipe = serde_json::from_str(json).expect("recipe parses");
    evaluate(&recipe).expect("recipe evaluates")
}

/// Two independent fBMs `a` and `b`, then `tail` (nodes reading them).
fn two_noises(tail: &str) -> String {
    format!(
        r#"{{ "resolution": 32, "nodes": [
          {{ "id": "a", "op": "noise", "kind": "fbm", "scale": 4, "octaves": 4 }},
          {{ "id": "b", "op": "noise", "kind": "fbm", "scale": 4, "octaves": 4, "seed": 1 }},
          {tail} ] }}"#
    )
}

#[test]
fn math_max_of_two_images_is_lighten_and_the_per_pixel_max() {
    let raw = |id: &str| {
        let lighten = r#"{ "id": "o", "op": "mix", "mode": "lighten", "inputs": ["a", "b"] }"#;
        let recipe: TextureRecipe = serde_json::from_str(&two_noises(lighten)).unwrap();
        evaluate(&recipe.with_output(id)).unwrap()
    };
    let (a, b) = (raw("a"), raw("b"));
    let max = bake(&two_noises(
        r#"{ "id": "o", "op": "math", "func": "max", "value": 9, "inputs": ["a", "b"] }"#,
    ));
    let lighten = bake(&two_noises(
        r#"{ "id": "o", "op": "mix", "mode": "lighten", "inputs": ["a", "b"] }"#,
    ));
    assert_eq!(max, lighten);
    for ((m, a), b) in max.pixels().iter().zip(a.pixels()).zip(b.pixels()) {
        assert_eq!(m[0], a[0].max(b[0]));
    }
    assert_ne!(a, b, "the two seeds give two patterns");
}

#[test]
fn darken_and_divide_combine_per_channel() {
    let img = bake(
        r#"{ "resolution": 2, "output": "o", "nodes": [
          { "id": "a", "op": "constant", "color": [0.2, 0.8, 0.6, 1] },
          { "id": "b", "op": "constant", "color": [0.5, 0.4, 0, 1] },
          { "id": "d", "op": "mix", "mode": "darken", "inputs": ["a", "b"] },
          { "id": "o", "op": "mix", "mode": "divide", "inputs": ["d", "b"] } ] }"#,
    );
    // darken → [0.2, 0.4, 0]; ÷ b → [0.4, 1, 0 (b = 0 leaves a)].
    assert_eq!(img.pixels()[0], [0.4, 1.0, 0.0, 1.0]);
}

#[test]
fn auto_levels_stretches_fbm_to_exactly_zero_one() {
    let img = bake(&two_noises(
        r#"{ "id": "o", "op": "auto_levels", "inputs": ["a"] }"#,
    ));
    let reds = img.pixels().iter().map(|p| p[0]);
    let (lo, hi) = reds.fold((1.0f32, 0.0f32), |(lo, hi), c| (lo.min(c), hi.max(c)));
    assert_eq!((lo, hi), (0.0, 1.0));
}

#[test]
fn auto_levels_keeps_hue_alpha_and_flat_input() {
    let img = bake(
        r#"{ "resolution": 2, "output": "o", "nodes": [
          { "id": "c", "op": "checker", "tiles": 2,
            "color_a": [0.2, 0.3, 0.4, 0.5], "color_b": [0.4, 0.5, 0.6, 0.5] },
          { "id": "o", "op": "auto_levels", "inputs": ["c"] } ] }"#,
    );
    let dark = img.pixels().iter().find(|p| p[0] == 0.0).unwrap();
    assert!((dark[2] - 0.5).abs() < 1e-6 && dark[3] == 0.5, "{dark:?}");
    let flat = bake(
        r#"{ "resolution": 2, "nodes": [
          { "id": "c", "op": "constant", "color": [0.3, 0.3, 0.3, 1] },
          { "id": "o", "op": "auto_levels", "inputs": ["c"] } ] }"#,
    );
    assert_eq!(flat.pixels()[0], [0.3, 0.3, 0.3, 1.0]);
}

const HEX: &str = r##"{ "resolution": 4, "nodes": [
  { "id": "c", "op": "constant", "color": "#808080" },
  { "id": "r", "op": "color_ramp", "inputs": ["c"], "stops": [
    { "pos": 0, "color": [0, 0, 0, 1] }, { "pos": 1, "color": "#ff000080" } ] } ] }"##;

#[test]
fn hex_colours_decode_to_linear_and_round_trip_as_arrays() {
    let recipe: TextureRecipe = serde_json::from_str(HEX).expect("hex parses");
    let json = recipe.to_json().unwrap();
    assert!(!json.contains('#'), "ToJson writes linear arrays: {json}");
    let again: TextureRecipe = serde_json::from_str(&json).unwrap();
    assert_eq!(again, recipe);
    let grey = evaluate(&recipe.with_output("c")).unwrap().pixels()[0];
    assert!((grey[0] - 0.216).abs() < 1e-3 && grey[3] == 1.0, "{grey:?}");
}

#[test]
fn a_malformed_hex_is_an_error_naming_the_param() {
    let bad = HEX.replace("#ff000080", "#ff00");
    let err = serde_json::from_str::<TextureRecipe>(&bad)
        .unwrap_err()
        .to_string();
    assert!(err.contains("color") && err.contains("#ff00"), "{err}");
}
