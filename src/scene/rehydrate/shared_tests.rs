//! A scene load instancing one file many times parses it once, and every instance
//! carries the import's revision (#857).

use glam::Vec3;

use crate::asset::import_cache::parses;
use crate::scene::asset_instance::instantiate_asset;
use crate::scene::asset_instance::lightmap_test_support::quad_model;
use crate::scene::{apply_scene_data, to_scene_data, Scene};

#[test]
fn a_load_parses_each_file_once_and_instances_share_a_revision() {
    let path = quad_model("rusty_shared_import");
    let mut scene = Scene::new();
    let reference = format!("{path}::Quad");
    let ids: Vec<u32> = (0..40)
        .map(|_| instantiate_asset(&mut scene, &reference, None, Vec3::ZERO).unwrap())
        .collect();
    let saved = to_scene_data(&scene);

    let before = parses();
    let mut loaded = Scene::new();
    apply_scene_data(&mut loaded, saved);
    assert_eq!(parses(), before, "the file was already imported: no parse");

    let revisions: Vec<_> = (loaded.world.ids_with_mesh().into_iter())
        .map(|id| loaded.world.mesh(id).unwrap().revision)
        .collect();
    assert_eq!(revisions.len(), ids.len());
    assert!(
        revisions.iter().all(|r| *r == revisions[0]),
        "one shared revision"
    );
    let spawned = scene.world.mesh(ids[0]).unwrap().revision;
    assert_eq!(revisions[0], spawned, "the same import as the spawn");
    assert!(!loaded.world.mesh(ids[0]).unwrap().vertices.is_empty());
}
