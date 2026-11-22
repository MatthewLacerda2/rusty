use rusty_asset::sidecar::texture;
use rusty_asset::MapRef;
use rusty_procgen::catalog::Constant;
use rusty_procgen::recipe::{Node, OpKind};
use rusty_procgen::TextureRecipe;

use super::*;

const SLOTS: [&str; 5] = ["base_color", "normal", "roughness", "metallic", "occlusion"];

fn panel(w: u32) -> MaterialAsset {
    let node = Node {
        id: "c".into(),
        op: OpKind::Constant(Constant {
            color: [0.5, 0.5, 1.0, 1.0],
        }),
        inputs: vec![],
    };
    let mut r = TextureRecipe::new(0, vec![node]).with_size([w, 4]);
    r.outputs = SLOTS.iter().map(|s| (s.to_string(), "c".into())).collect();
    MaterialAsset {
        maps_recipe: Some(r),
        ..MaterialAsset::default()
    }
}

fn sizes(m: &MaterialAsset) -> Vec<(u32, u32)> {
    let maps = [
        &m.base_color_map,
        &m.normal_map,
        &m.roughness_map,
        &m.metallic_map,
        &m.occlusion_map,
    ];
    let file = |map: &Option<MapRef>| map.as_ref().unwrap().file().unwrap();
    maps.map(|map| {
        let file = file(map);
        (
            image::image_dimensions(&file).unwrap().0,
            texture(&file).max_size,
        )
    })
    .to_vec()
}

#[test]
fn a_rebake_at_4096_ships_every_map_at_4096() {
    let dir = rusty_test_support::temp::dir().join("maps_ship_size");
    let mut m = panel(256);
    bake_maps(&mut m, "panel", &dir, &dir).unwrap();
    assert_eq!(sizes(&m), vec![(256, 2048); 5], "the first bake fits");
    let mut materials = MaterialLibrary::from([("panel".into(), m)]);
    rebake_maps(&mut materials, "panel", Some(4096), &dir, &dir).unwrap();
    assert_eq!(sizes(&materials["panel"]), vec![(4096, 4096); 5]);
}

#[test]
fn a_rebake_above_8192_is_refused_and_writes_nothing() {
    let dir = rusty_test_support::temp::dir().join("maps_too_large");
    let mut materials = MaterialLibrary::from([("panel".into(), panel(64))]);
    let err = rebake_maps(&mut materials, "panel", Some(16384), &dir, &dir).unwrap_err();
    assert!(err.contains("largest Max Size, 8192"), "{err}");
    assert!(!dir.exists(), "nothing written");
    assert!(
        materials["panel"].base_color_map.is_none(),
        "the library unchanged"
    );
}
