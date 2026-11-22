//! Moving a recess's shade from the albedo into the occlusion map bakes the same bounce; a material without one bakes as before.

use glam::Vec3;

use super::run_tests::panel;
use super::tests::{mean, settings};
use super::*;
use rusty_asset::MapRef;
use rusty_components::{LightComponent, MaterialAsset, MaterialComponent};
use rusty_scene::test_levels::block;
use rusty_scene::Scene;

/// A fake map reader: `a.png` (the albedo) averages 0.8 as a colour map, `ao.png`
/// 0.5 as a data map, and `ao_a.png` their product as a colour map; a map read as
/// the wrong encoding fails the test.
fn averages(map: &MapRef, encoding: MapEncoding) -> Option<Vec3> {
    let want = match map.as_str() {
        "a.png" => (MapEncoding::Colour, 0.8),
        "ao.png" => (MapEncoding::Data, 0.5),
        "ao_a.png" => (MapEncoding::Colour, 0.4),
        "white.png" => (MapEncoding::Data, 1.0),
        other => panic!("no map {other}"),
    };
    assert_eq!(encoding, want.0, "{map:?} read as {encoding:?}");
    Some(Vec3::splat(want.1))
}

fn material(albedo: &str, occlusion: Option<&str>, strength: f32) -> MaterialAsset {
    MaterialAsset {
        base_color_map: Some(albedo.into()),
        occlusion_map: occlusion.map(MapRef::from),
        occlusion_strength: strength,
        ..MaterialAsset::default()
    }
}

fn gathered(m: MaterialAsset) -> Vec3 {
    let mut scene = Scene::new();
    scene.materials.insert("m".to_string(), m);
    panel(&mut scene, Some(&["m"]));
    BakeScene::gather(&scene, &averages).meshes[0].runs[0].albedo
}

#[test]
fn the_occlusion_map_scales_the_albedo_by_its_strength() {
    let close = |a: Vec3, b: f32| (a - Vec3::splat(b)).abs().max_element() < 1e-6;
    let albedo = gathered(material("a.png", Some("ao.png"), 1.0));
    assert!(close(albedo, 0.4), "{albedo}");
    let albedo = gathered(material("a.png", Some("ao.png"), 0.5));
    assert!(close(albedo, 0.6), "half strength lerps halfway: {albedo}");
    let none = gathered(material("a.png", Some("ao.png"), 0.0));
    assert_eq!(none, Vec3::splat(0.8), "strength 0 ignores the map");
    assert_eq!(gathered(material("a.png", None, 1.0)), Vec3::splat(0.8));
    // Read as the renderer reads it (`MaterialAsset::occlusion_amount`).
    let nan = gathered(material("a.png", Some("ao.png"), f32::NAN));
    assert!(
        close(nan, 0.4),
        "a NaN strength applies the map whole: {nan}"
    );
    let over = gathered(material("a.png", Some("ao.png"), 4.0));
    assert!(close(over, 0.4), "a strength past 1 is held at 1: {over}");
}

/// Lit only by the bounce from a Mixed lamp (its direct part is not baked); returns the ceiling's mean lightmap value.
fn ceiling(floor: MaterialAsset) -> f32 {
    let mut scene = Scene::new();
    scene.ambient_intensity = 0.0;
    scene.materials.insert("floor".to_string(), floor);
    let wear = |scene: &mut Scene, id: u32, key: &str| {
        let materials = (key != "white").then(|| vec![key.to_string()]);
        let materials = materials.map(|materials| MaterialComponent { materials });
        scene.world.set_material(id, materials);
    };
    let floor = block(
        &mut scene,
        "Floor",
        Vec3::new(0.0, -0.2, 0.0),
        Vec3::new(4.0, 0.0, 4.0),
    );
    wear(&mut scene, floor, "floor");
    let top = block(
        &mut scene,
        "Ceiling",
        Vec3::new(0.0, 3.0, 0.0),
        Vec3::new(4.0, 3.2, 4.0),
    );
    for (lo, hi) in [
        (Vec3::new(-0.2, 0.0, 0.0), Vec3::new(0.0, 3.0, 4.0)),
        (Vec3::new(4.0, 0.0, 0.0), Vec3::new(4.2, 3.0, 4.0)),
        (Vec3::new(0.0, 0.0, -0.2), Vec3::new(4.0, 3.0, 0.0)),
        (Vec3::new(0.0, 0.0, 4.0), Vec3::new(4.0, 3.0, 4.2)),
    ] {
        block(&mut scene, "Wall", lo, hi);
    }
    let lamp = scene.add_entity("Lamp".to_string());
    scene.world.transform_mut(lamp).expect("an entity").position = Vec3::new(2.0, 0.5, 2.0);
    let light = LightComponent {
        intensity: 4.0,
        mode: rusty_components::LightMode::Mixed,
        ..rusty_scene::create::default_light()
    };
    scene.world.set_light(lamp, Some(light));
    let maps = bake(&BakeScene::gather(&scene, &averages), &settings());
    mean(&maps, top)
}

#[test]
fn a_shade_moved_into_the_occlusion_map_bounces_the_same_light() {
    let in_albedo = ceiling(material("ao_a.png", None, 1.0));
    let split = ceiling(material("a.png", Some("ao.png"), 1.0));
    let unshaded = ceiling(material("a.png", None, 1.0));
    assert!(in_albedo > 0.0, "the floor's bounce reaches the ceiling");
    let off = (split - in_albedo).abs() / in_albedo;
    assert!(off < 0.02, "A×O {in_albedo} against A with O {split}");
    assert!(
        unshaded > 1.3 * split,
        "the shade still darkens: {unshaded} vs {split}"
    );
}

#[test]
fn a_white_occlusion_map_bakes_the_same_texels_as_none() {
    let bake_of = |m: MaterialAsset| {
        let mut scene = super::run_tests::library();
        scene.materials.insert("m".to_string(), m);
        panel(&mut scene, Some(&["m"]));
        let mut baked = BakeScene::gather(&scene, &averages);
        baked.meshes.push(super::tests::floor());
        baked.lights.push(super::tests::sun(true));
        bake(&baked, &settings())
    };
    let none = bake_of(material("a.png", None, 1.0));
    assert_eq!(bake_of(material("a.png", Some("white.png"), 1.0)), none);
    assert_eq!(bake_of(material("a.png", Some("ao.png"), 0.0)), none);
    assert_ne!(bake_of(material("a.png", Some("ao.png"), 1.0)), none);
}
