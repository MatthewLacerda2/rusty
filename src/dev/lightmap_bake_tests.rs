//! The lightmap bake as an authoring action (#438): files written beside the scene,
//! referenced from it, byte-identical per seed, and surviving a save / load.

use super::*;
use crate::scene::authoring::{primitive_mesh_component, Primitive};

/// A scene with a static floor Plane, a static Sphere (no lightmap UV) and a dynamic
/// Box; returns it with the floor's and the sphere's ids.
fn scene() -> (Scene, u32, u32) {
    let mut scene = Scene::new();
    let mut add = |name: &str, primitive, is_static| {
        let id = scene.add_entity(name.to_string());
        scene
            .world
            .set_mesh(id, primitive_mesh_component(primitive));
        scene.world.set_static(id, is_static);
        id
    };
    let floor = add("Floor", Primitive::Plane, true);
    let ball = add("Ball", Primitive::Sphere, true);
    add("Crate", Primitive::Box, false);
    (scene, floor, ball)
}

fn quick() -> BakeSettings {
    BakeSettings {
        texels_per_unit: 2.0,
        samples: 8,
        bounces: 1,
        seed: 1,
        max_resolution: 16,
    }
}

/// A fresh temp dir and a scene path inside it.
fn scene_path(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("rusty_lightmap_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("level.scene").to_string_lossy().into_owned()
}

#[test]
fn bakes_the_lightmappable_static_mesh_and_references_its_file() {
    let (mut scene, floor, ball) = scene();
    let path = scene_path("refs");
    assert_eq!(
        bake_scene_lightmaps(&mut scene, Some(&path), &quick()),
        Ok(1)
    );
    let file = scene
        .lightmaps
        .get(floor)
        .expect("the floor is lightmapped");
    assert!(
        file.ends_with("level.scene.lightmaps/lightmap_1.png"),
        "{file}"
    );
    assert!(
        scene.lightmaps.get(ball).is_none(),
        "no UV2: probes, not a lightmap"
    );
    let png = image::open(file).unwrap().to_rgba8();
    assert!(png.width() >= 4 && png.width() == png.height());
}

#[test]
fn the_same_seed_writes_the_same_bytes() {
    let (mut scene, floor, _) = scene();
    let path = scene_path("bytes");
    bake_scene_lightmaps(&mut scene, Some(&path), &quick()).unwrap();
    let first = std::fs::read(scene.lightmaps.get(floor).unwrap()).unwrap();
    bake_scene_lightmaps(&mut scene, Some(&path), &quick()).unwrap();
    let second = std::fs::read(scene.lightmaps.get(floor).unwrap()).unwrap();
    assert_eq!(first, second);
}

#[test]
fn a_rebake_drops_lightmaps_that_no_longer_apply() {
    let (mut scene, floor, _) = scene();
    let path = scene_path("stale");
    bake_scene_lightmaps(&mut scene, Some(&path), &quick()).unwrap();
    let old = scene.lightmaps.get(floor).unwrap().to_string();
    scene.world.set_static(floor, false);
    assert_eq!(
        bake_scene_lightmaps(&mut scene, Some(&path), &quick()),
        Ok(0)
    );
    assert!(scene.lightmaps.is_empty());
    assert!(!Path::new(&old).exists(), "the stale file is removed");
}

#[test]
fn references_survive_a_save_and_load() {
    let (mut scene, floor, _) = scene();
    let path = scene_path("save");
    bake_scene_lightmaps(&mut scene, Some(&path), &quick()).unwrap();
    scene.save_to_file(&path).unwrap();
    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    assert_eq!(loaded.lightmaps, scene.lightmaps);
    assert!(loaded.lightmaps.get(floor).is_some());
}

#[test]
fn an_unsaved_scene_cannot_bake() {
    let (mut scene, _, _) = scene();
    assert!(bake_scene_lightmaps(&mut scene, None, &quick()).is_err());
}
