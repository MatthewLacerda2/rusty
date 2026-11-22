use glam::Vec2;
use serde_json::{json, Value};

use crate::ops::outline::fill::{self, Shape};
use crate::recipe::{Node, TextureRecipe};
use crate::{evaluate, evaluate_with, Image, RunError, Sources};

use super::path::FillRule;

fn recipe(op: Value) -> Result<TextureRecipe, String> {
    let mut doc = op;
    doc["id"] = json!("n");
    let node: Node = serde_json::from_value(doc).map_err(|e| e.to_string())?;
    Ok(TextureRecipe::new(64, vec![node]))
}

fn bake(op: Value) -> Image {
    evaluate(&recipe(op).unwrap()).unwrap()
}

fn red(img: &Image) -> Vec<f32> {
    img.pixels().iter().map(|p| p[0]).collect()
}

fn max_diff(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f32::max)
}

#[test]
fn one_glyph_is_its_outline_filled_where_the_layout_centres_it() {
    let font = rusty_font::default_font();
    let glyph = font.glyph('R').unwrap();
    let upem = font.units_per_em();
    let size = 0.5;
    let s = size / upem;
    let advance = rusty_font::shape(&font, "R", &[])[0].advance;
    let [ascent, descent, _] = font.vertical_metrics(size);
    let origin = Vec2::new(
        0.5 - advance * s / 2.0,
        0.5 - (ascent + descent) / 2.0 + ascent,
    );
    let place = |p: Vec2| origin + Vec2::new(p.x, -p.y) * s;
    let outline = font.outline(glyph, upem).unwrap();
    let shape: Shape = outline
        .iter()
        .map(|[a, b]| [place(*a), place(*b)])
        .collect();
    let direct = fill::coverage(&[shape], 64.into(), FillRule::Nonzero);
    let baked = red(&bake(json!({ "op": "text", "text": "R", "size": size })));
    assert!(max_diff(&baked, &direct) < 1e-6);
    assert!(baked.iter().sum::<f32>() > 100.0, "the glyph is drawn");
}

#[test]
fn a_path_rectangle_is_shapes_rect() {
    let rect = bake(json!({ "op": "shape", "kind": "rect", "size": [0.5, 0.25] }));
    let path = bake(json!({ "op": "path", "d": "M.25 .375 H.75 V.625 H.25 Z" }));
    assert!(max_diff(&red(&rect), &red(&path)) < 1e-5);
    let stroked = bake(json!({ "op": "path", "d": "M.25 .375 H.75 V.625 H.25 Z", "stroke": 0.05 }));
    let ring = red(&stroked);
    assert_eq!(ring[32 * 64 + 32], 0.0, "a stroke leaves the inside open");
    assert_eq!(ring[24 * 64 + 32], 1.0, "and covers the outline");
}

#[test]
fn evenodd_cuts_the_hole_nonzero_keeps_it_filled() {
    let d = "M.1 .1 H.9 V.9 H.1 Z M.3 .3 H.7 V.7 H.3 Z";
    let centre =
        |rule: &str| red(&bake(json!({ "op": "path", "d": d, "fill_rule": rule })))[32 * 64 + 32];
    assert_eq!(centre("evenodd"), 0.0);
    assert_eq!(centre("nonzero"), 1.0);
}

#[test]
fn a_malformed_path_is_refused_naming_the_node_and_byte() {
    let e = recipe(json!({ "op": "path", "d": "M0 0 L0.5 oops" })).unwrap_err();
    assert!(
        e.contains(r#"node "n" (path)"#) && e.contains("at byte 10"),
        "{e}"
    );
    let pathological = "M0 0 ".to_string() + &"A4 4 0 1 1 0 0.0001 ".repeat(3000);
    let e = recipe(json!({ "op": "path", "d": pathological })).unwrap_err();
    assert!(e.contains(r#"node "n" (path)"#), "{e}");
}

#[test]
fn a_font_nobody_handed_in_is_refused_before_anything_bakes() {
    let r = recipe(json!({ "op": "text", "text": "A", "font": "fonts/stencil.ttf" })).unwrap();
    assert_eq!(r.font_paths(), [("n", "fonts/stencil.ttf")]);
    let e = evaluate(&r).unwrap_err();
    assert!(matches!(&e, RunError::Node(n) if n.node == "n"), "{e:?}");
    assert!(
        e.to_string()
            .contains(r#"has no font loaded for "fonts/stencil.ttf""#),
        "{e}"
    );
    // The default font's bytes, handed in under another path, bake what the default does.
    let mut sources = Sources::new();
    let bytes = std::fs::read(
        rusty_core::project::checkout().join("engine/fonts/InstrumentSans-Regular.ttf"),
    );
    sources
        .insert_font("fonts/stencil.ttf", bytes.unwrap())
        .unwrap();
    let handed = evaluate_with(&r, &sources).unwrap();
    assert_eq!(handed, bake(json!({ "op": "text", "text": "A" })));
    assert!(recipe(json!({ "op": "text", "text": "x\n".repeat(300) })).is_err());
}

#[test]
fn a_size_recipe_and_a_wild_size_bake_without_trouble() {
    let mut r =
        recipe(json!({ "op": "text", "text": "WIDE TEXT", "size": 1e30, "tracking": f32::MAX }))
            .unwrap();
    r = r.with_size([32, 8]);
    let img = evaluate(&r).unwrap();
    assert!(img
        .pixels()
        .iter()
        .all(|p| p[0].is_finite() && (0.0..=1.0).contains(&p[0])));
    let none = bake(json!({ "op": "text", "text": "A", "size": 0.0 }));
    assert!(red(&none).iter().all(|v| *v == 0.0));
}
