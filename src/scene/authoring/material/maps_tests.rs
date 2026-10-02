//! Tests for baking a material's own maps recipe (#409).

use std::path::{Path, PathBuf};

use super::*;
use crate::procgen::recipe::{Node, OpKind};
use crate::procgen::TextureRecipe;

/// A checker recipe with named `outputs` (slot → the one node, `c`).
fn recipe(resolution: u32, slots: &[&str]) -> TextureRecipe {
    let mut r = TextureRecipe::new(
        resolution,
        vec![Node {
            id: "c".into(),
            op: OpKind::Checker {
                tiles: 2,
                color_a: [0.0, 0.0, 0.0, 1.0],
                color_b: [1.0, 1.0, 1.0, 1.0],
            },
            inputs: vec![],
        }],
    );
    r.outputs = slots.iter().map(|s| (s.to_string(), "c".into())).collect();
    r
}

fn with_maps(recipe: TextureRecipe) -> MaterialAsset {
    MaterialAsset {
        maps_recipe: Some(recipe),
        ..MaterialAsset::default()
    }
}

fn dir(test: &str) -> PathBuf {
    crate::test_temp::dir().join(test)
}

fn png(dir: &Path, file: &str) -> String {
    format!("{}/{file}", dir.display())
}

fn width(path: &str) -> u32 {
    image::image_dimensions(path).expect("baked PNG decodes").0
}

#[test]
fn bake_fills_each_slots_maps_and_a_packed_map_fills_both() {
    let dir = dir("maps_fill");
    let mut m = with_maps(recipe(8, &["albedo", "normal", "mr"]));
    bake_maps(&mut m, "panel", &dir).unwrap();
    let mr = png(&dir, "panel_metallic_roughness.png");
    assert_eq!(m.base_color_map, Some(png(&dir, "panel_base_color.png")));
    assert_eq!(m.normal_map, Some(png(&dir, "panel_normal.png")));
    assert_eq!(m.metallic_map.as_ref(), Some(&mr));
    assert_eq!(m.roughness_map.as_ref(), Some(&mr));
    assert_eq!(m.emissive_map, None, "no emissive output, no emissive map");
    assert_eq!(width(&mr), 8);
}

#[test]
fn a_hand_set_map_path_wins_over_the_bake() {
    let dir = dir("maps_explicit");
    let mut m = with_maps(recipe(8, &["base_color", "normal"]));
    m.normal_map = Some("hand_made_n.png".into());
    bake_maps(&mut m, "panel", &dir).unwrap();
    assert_eq!(m.normal_map.as_deref(), Some("hand_made_n.png"));
    assert_eq!(m.base_color_map, Some(png(&dir, "panel_base_color.png")));
}

#[test]
fn no_recipe_bakes_nothing_and_a_path_like_name_is_refused() {
    let mut plain = MaterialAsset::default();
    bake_maps(&mut plain, "../x", &dir("maps_none")).unwrap();
    assert_eq!(plain.base_color_map, None);

    let mut m = with_maps(recipe(8, &["base_color"]));
    let err = bake_maps(&mut m, "a/b", &dir("maps_bad_name")).unwrap_err();
    assert!(err.contains("\"a/b\""), "{err}");
    assert_eq!(m.base_color_map, None);
}

#[test]
fn rebake_at_a_new_resolution_rewrites_the_maps_and_keeps_it() {
    let dir = dir("maps_rebake");
    let mut materials = MaterialLibrary::new();
    let mut m = with_maps(recipe(8, &["base_color"]));
    bake_maps(&mut m, "panel", &dir).unwrap();
    materials.insert("panel".into(), m);

    rebake_maps(&mut materials, "panel", Some(32), &dir).unwrap();
    let m = &materials["panel"];
    assert_eq!(m.maps_recipe.as_ref().unwrap().resolution, 32);
    assert_eq!(width(m.base_color_map.as_deref().unwrap()), 32);
}

#[test]
fn a_failed_rebake_leaves_the_library_unchanged() {
    let dir = dir("maps_rebake_fail");
    let mut materials = MaterialLibrary::from([
        ("panel".into(), with_maps(recipe(8, &["base_color"]))),
        ("plain".into(), MaterialAsset::default()),
    ]);
    assert!(rebake_maps(&mut materials, "panel", Some(0), &dir).is_err());
    assert_eq!(
        materials["panel"].maps_recipe.as_ref().unwrap().resolution,
        8
    );
    assert_eq!(materials["panel"].base_color_map, None);

    let err = rebake_maps(&mut materials, "plain", None, &dir).unwrap_err();
    assert!(err.contains("no maps recipe"), "{err}");
    let err = rebake_maps(&mut materials, "nope", None, &dir).unwrap_err();
    assert!(err.contains("no material named \"nope\""), "{err}");
}

#[test]
fn the_recipe_travels_with_the_asset_as_maps() {
    let m = with_maps(recipe(16, &["base_color"]));
    let json = serde_json::to_value(&m).unwrap();
    assert!(json.get("maps").is_some(), "{json}");
    let back: MaterialAsset = serde_json::from_value(json).unwrap();
    assert_eq!(back.maps_recipe, m.maps_recipe);
    let plain = serde_json::to_value(MaterialAsset::default()).unwrap();
    assert!(plain.get("maps").is_none(), "{plain}");
}
