use glam::Vec3;

use crate::asset_instance::instantiate_asset;
use crate::serialize::{apply_scene_data, to_scene_data};
use crate::snapshot::SceneSnapshot;
use rusty_asset::import_cache::lookups;
use rusty_asset::mesh_data::Shared;
use rusty_components::mesh::copies;
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::rehydrate::rebuilds;
use rusty_scene::Scene;
use rusty_test_support::models::{quad_file, quad_mode_gltf};

fn level(dir: &str, n: usize) -> (Scene, Vec<u32>, String) {
    let path = quad_file(dir);
    let mut scene = Scene::new();
    let parts = (0..n)
        .map(|_| {
            let root = instantiate_asset(&mut scene, &path, None, Vec3::ZERO).unwrap();
            let mut tree = vec![root];
            tree.extend(scene.world.children(root));
            *tree
                .iter()
                .find(|&&id| scene.world.mesh(id).is_some())
                .unwrap()
        })
        .collect();
    (scene, parts, path)
}

fn shares_import(scene: &Scene, id: u32) -> bool {
    let mesh = scene.world.mesh(id).unwrap();
    let reference = mesh.asset_ref.as_deref().unwrap();
    let (file, sub) = reference.rsplit_once("::").unwrap();
    let import = rusty_asset::imported(std::path::Path::new(file)).unwrap();
    let sub = import.sub_mesh(sub).unwrap();
    Shared::same(&mesh.vertices, &sub.vertices) && Shared::same(&mesh.indices, &sub.indices)
}

fn indices(scene: &Scene, id: u32) -> Vec<u32> {
    scene.world.mesh(id).unwrap().indices.to_vec()
}

fn copied(from: (usize, usize)) -> (usize, usize) {
    (rebuilds() - from.0, copies() - from.1)
}

fn untouched_stop(dir: &str, n: usize) -> (usize, (usize, usize)) {
    let (mut scene, parts, _) = level(dir, n);
    let revisions: Vec<_> = parts
        .iter()
        .map(|&id| scene.world.mesh(id).unwrap().revision)
        .collect();
    let snapshot = SceneSnapshot::capture(&scene);
    let (looked, from) = (lookups(), (rebuilds(), copies()));
    snapshot.restore(&mut scene);
    let cost = (lookups() - looked, copied(from));
    for (i, &id) in parts.iter().enumerate() {
        assert_eq!(indices(&scene, id), vec![0, 1, 2, 0, 2, 3], "part {id}");
        let revision = scene.world.mesh(id).unwrap().revision;
        assert_eq!(revision, revisions[i], "part {id}: the live geometry, kept");
        assert!(shares_import(&scene, id), "part {id}: the import's buffers");
    }
    cost
}

#[test]
fn stop_over_linked_models_looks_the_file_up_once_and_copies_no_instance() {
    let one = untouched_stop("rusty_stop_linked_one", 1);
    let many = untouched_stop("rusty_stop_linked_many", 20);
    assert_eq!(many.0, 1, "one lookup of the model file for the whole Stop");
    assert_eq!(
        many.1, one.1,
        "the model's own copies, never one per instance"
    );
    assert_eq!(many.1 .1, 0, "no geometry buffer is copied");
}

#[test]
fn stop_rebuilds_an_instance_whose_override_re_meshes_it() {
    let (mut scene, parts, _) = level("rusty_stop_linked_override", 3);
    let sphere = create_entity(&mut scene, "S", Some(Primitive::Sphere));
    let sphere = scene.world.mesh(sphere).unwrap().clone();
    scene.world.set_mesh(parts[1], Some(sphere.clone()));
    let snapshot = SceneSnapshot::capture(&scene);
    let looked = lookups();
    snapshot.restore(&mut scene);
    assert_eq!(lookups() - looked, 1, "the model file, once");
    assert_eq!(
        indices(&scene, parts[1]),
        sphere.indices.to_vec(),
        "the override's sphere"
    );
    assert_eq!(indices(&scene, parts[0]), vec![0, 1, 2, 0, 2, 3]);
}

#[test]
fn a_load_of_linked_models_looks_the_file_up_once() {
    let (scene, parts, _) = level("rusty_load_linked", 20);
    let data = to_scene_data(&scene);
    let mut loaded = Scene::new();
    let (looked, from) = (lookups(), (rebuilds(), copies()));
    apply_scene_data(&mut loaded, data);
    assert_eq!(lookups() - looked, 1, "one lookup of the model file");
    assert_eq!(copied(from), (parts.len() + 2, 0), "no copy per instance");
    for &id in &parts {
        assert_eq!(indices(&loaded, id), vec![0, 1, 2, 0, 2, 3], "part {id}");
        assert!(
            shares_import(&loaded, id),
            "part {id}: the import's buffers"
        );
    }
}

#[test]
fn stop_reloads_a_model_rewritten_during_play_with_no_note() {
    let (mut scene, parts, path) = level("rusty_stop_linked_rewritten", 3);
    let snapshot = SceneSnapshot::capture(&scene);
    let (json, bin) = quad_mode_gltf(4, &[0, 1, 2]);
    let dir = std::path::Path::new(&path).parent().unwrap();
    std::fs::write(dir.join("quad.bin"), bin).unwrap();
    std::fs::write(&path, json).unwrap();
    snapshot.restore(&mut scene);
    for &id in &parts {
        assert_eq!(
            indices(&scene, id),
            vec![0, 1, 2],
            "part {id}: the new export"
        );
    }
}
