use crate::model_writes::ModelWrites;
use rusty_asset::mesh_data::Shared;
use rusty_components::mesh::copies;
use rusty_core::fs::written::write;
use rusty_scene::{asset_mesh_component, Scene};

const QUAD: &str = "o Model\nv 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3\nf 1 3 4\n";
const TRIANGLE: &str = "o Model\nv 0 0 0\nv 2 0 0\nv 2 2 0\nf 1 2 3\n";

pub(super) fn level(dir: &str, n: usize) -> (Scene, Vec<u32>, String) {
    let dir = rusty_test_support::temp::dir().join(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("model.obj").to_string_lossy().into_owned();
    write(std::path::Path::new(&path), QUAD.as_bytes()).unwrap();
    let mut scene = Scene::new();
    let ids = (0..n)
        .map(|i| {
            let id = scene.add_entity(format!("Model {i}"));
            let mesh = asset_mesh_component(&format!("{path}::Model"));
            scene.world.set_mesh(id, Some(mesh));
            id
        })
        .collect();
    (scene, ids, path)
}

pub(super) fn cached(path: &str) -> (Shared<rusty_asset::Vertex>, Shared<u32>) {
    let import = rusty_asset::imported(std::path::Path::new(path)).unwrap();
    let sub = import.sub_mesh("Model").unwrap();
    (sub.vertices.clone(), sub.indices.clone())
}

pub(super) fn shares(
    scene: &Scene,
    id: u32,
    buffers: &(Shared<rusty_asset::Vertex>, Shared<u32>),
) -> bool {
    let mesh = scene.world.mesh(id).unwrap();
    Shared::same(&mesh.vertices, &buffers.0) && Shared::same(&mesh.indices, &buffers.1)
}

#[test]
fn a_load_of_many_instances_shares_the_caches_buffers_and_copies_none() {
    let (mut scene, _, path) = level("rusty_sharing_load", 8);
    let data = crate::to_scene_data(&scene);
    let from = copies();
    crate::apply_scene_data(&mut scene, data);
    assert_eq!(copies() - from, 0, "no vertex or index buffer is copied");
    let buffers = cached(&path);
    let ids = scene.world.ids();
    assert_eq!(ids.len(), 8);
    for &id in ids.iter() {
        assert!(
            shares(&scene, id, &buffers),
            "entity {id}: the cache's buffers"
        );
    }
}

#[test]
fn a_reimport_reaches_every_instance_and_they_share_it() {
    let (mut scene, ids, path) = level("rusty_sharing_reimport", 3);
    let mut writes = ModelWrites::default();
    write(std::path::Path::new(&path), TRIANGLE.as_bytes()).unwrap();
    assert_eq!(writes.follow(&mut scene), ids.len());
    let buffers = cached(&path);
    assert_eq!(buffers.1.len(), 3, "the new file's triangle");
    for id in ids {
        assert!(
            shares(&scene, id, &buffers),
            "entity {id}: the new import's buffers"
        );
    }
}

#[test]
fn an_undo_that_keeps_the_live_mesh_keeps_it_shared() {
    let (mut scene, ids, path) = level("rusty_sharing_undo", 2);
    scene.begin_edits();
    scene.world.transform_mut(ids[0]).unwrap().position.x = 3.0;
    scene.commit_edits(None);
    let from = copies();
    assert!(scene.undo().is_some());
    assert_eq!(
        copies() - from,
        0,
        "the live geometry is adopted, not copied"
    );
    let buffers = cached(&path);
    for &id in ids.iter() {
        assert!(
            shares(&scene, id, &buffers),
            "entity {id}: the cache's buffers"
        );
    }
}

#[test]
fn play_then_stop_leaves_every_instance_on_the_caches_buffers() {
    let (mut scene, ids, path) = level("rusty_sharing_stop", 4);
    let snapshot = crate::snapshot::SceneSnapshot::capture(&scene);
    snapshot.restore(&mut scene);
    let buffers = cached(&path);
    for &id in &ids {
        assert!(
            shares(&scene, id, &buffers),
            "entity {id}: the cache's buffers, not a copy"
        );
    }
}
