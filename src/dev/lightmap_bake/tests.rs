//! The lightmap bake as an authoring action (#438): files written beside the scene,
//! referenced from it, byte-identical per seed, and surviving a save / load.

use super::*;
use crate::scene::authoring::{primitive_mesh_component, Primitive};

/// A scene with a static floor Plane, a static Sphere stripped of its lightmap UV (an
/// imported mesh without `TEXCOORD_1`) and a dynamic Box; returns it with the floor's
/// and the sphere's ids.
pub(super) fn scene() -> (Scene, u32, u32) {
    let mut scene = Scene::new();
    let mut add = |name: &str, mut mesh: Option<crate::scene::MeshComponent>, is_static| {
        let id = scene.add_entity(name.to_string());
        if name == "Ball" {
            let ball = mesh.as_mut().expect("a sphere mesh");
            ball.vertices
                .iter_mut()
                .for_each(|v| v.lightmap_uv = [0.0; 2]);
        }
        scene.world.set_mesh(id, mesh);
        scene.world.set_static(id, is_static);
        id
    };
    let floor = add("Floor", primitive_mesh_component(Primitive::Plane), true);
    let ball = add("Ball", primitive_mesh_component(Primitive::Sphere), true);
    add("Crate", primitive_mesh_component(Primitive::Box), false);
    (scene, floor, ball)
}

pub(super) fn quick() -> BakeSettings {
    BakeSettings {
        texels_per_unit: 2.0,
        samples: 8,
        bounces: 1,
        seed: 1,
        max_resolution: 16,
        filter_radius: 1,
    }
}

/// A fresh temp dir and a scene path inside it.
pub(super) fn scene_path(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("rusty_lightmap_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("level.scene").to_string_lossy().into_owned()
}

#[test]
fn bakes_the_lightmappable_static_mesh_and_references_its_page() {
    let (mut scene, floor, ball) = scene();
    let path = scene_path("refs");
    assert_eq!(
        bake_scene_lightmaps(&mut scene, Some(&path), &quick()),
        Ok(1)
    );
    let entry = scene
        .lightmaps
        .get(floor)
        .expect("the floor is lightmapped");
    let file = &scene.lightmaps.pages[entry.page as usize];
    let name = file.rsplit('/').next().unwrap();
    assert!(file.contains("level.scene.lightmaps/"), "{file}");
    assert!(
        name.starts_with("lightmap_0_") && name.ends_with(".png"),
        "{name}"
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
    assert!(scene.lightmaps.get(floor).is_some());
    let first = std::fs::read(&scene.lightmaps.pages[0]).unwrap();
    bake_scene_lightmaps(&mut scene, Some(&path), &quick()).unwrap();
    let second = std::fs::read(&scene.lightmaps.pages[0]).unwrap();
    assert_eq!(first, second);
}

#[test]
fn a_rebake_drops_lightmaps_that_no_longer_apply() {
    let (mut scene, floor, _) = scene();
    let path = scene_path("stale");
    bake_scene_lightmaps(&mut scene, Some(&path), &quick()).unwrap();
    let old = scene.lightmaps.pages[0].clone();
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

#[test]
fn the_bake_counts_static_meshes_left_without_a_lightmap_uv() {
    let (mut scene, _, _) = scene();
    let input = gather_bake_input(&mut scene);
    let maps = bake(&input, &quick());
    let report = missing_uv_report(input.meshes.len(), maps.len());
    assert_eq!(
        report.as_deref(),
        Some("1 static mesh has no lightmap UV — enable Generate Lightmap UVs")
    );
    assert_eq!(missing_uv_report(2, 2), None);
    assert!(missing_uv_report(3, 0)
        .unwrap()
        .starts_with("3 static meshes have"));
}
