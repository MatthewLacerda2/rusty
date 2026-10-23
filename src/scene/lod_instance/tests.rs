//! Tests for the `_LOD<n>` import convention (#472): a set's base name spawns one
//! LODGroup entity whose levels list its children, the whole-file spawn groups sets
//! and leaves other sub-objects alone, and the group's renderer references survive a
//! prefab extract / stamp.

use glam::Vec3;

use super::instantiate_model;
use crate::scene::authoring::instantiate_asset;
use crate::scene::{extract_prefab, instantiate_prefab, Scene};

/// Three meshes over one shared triangle (extent 1 m): two placed by nodes named
/// `Crate_LOD1` and `Crate_LOD0` (the Blender object names; the mesh names are
/// Blender's `Cube.*` data names), and a plain `Barrel`.
const LOD_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "scene": 0,
  "scenes": [ { "nodes": [ 0, 1, 2 ] } ],
  "nodes": [
    { "mesh": 0, "name": "Crate_LOD1" },
    { "mesh": 1, "name": "Crate_LOD0" },
    { "mesh": 2, "name": "Barrel" }
  ],
  "meshes": [
    { "name": "Cube.001", "primitives": [ { "attributes": { "POSITION": 0 } } ] },
    { "name": "Cube.000", "primitives": [ { "attributes": { "POSITION": 0 } } ] },
    { "name": "Barrel", "primitives": [ { "attributes": { "POSITION": 0 } } ] }
  ],
  "accessors": [
    { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
      "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 0.0] }
  ],
  "bufferViews": [ { "buffer": 0, "byteOffset": 0, "byteLength": 36 } ],
  "buffers": [ { "byteLength": 36,
    "uri": "data:application/octet-stream;base64,AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8AAAAA" } ]
}
"#;

fn write_gltf(name: &str) -> String {
    let path = crate::test_temp::dir().join(name);
    std::fs::write(&path, LOD_GLTF).unwrap();
    path.to_string_lossy().into_owned()
}

/// Each level's renderers' names, finest first.
fn level_names(scene: &Scene, group: u32) -> Vec<Vec<String>> {
    let g = scene.world.lod_group(group).expect("an LODGroup").clone();
    let name = |id: u32| scene.world.name(id).unwrap().clone();
    g.levels
        .iter()
        .map(|l| l.renderers.iter().map(|&r| name(r)).collect())
        .collect()
}

#[test]
fn the_base_name_spawns_one_group_over_its_levels() {
    let path = write_gltf("rusty_472_base.gltf");
    let mut scene = Scene::new();
    let pos = Vec3::new(3.0, 0.0, -2.0);
    let group = instantiate_asset(&mut scene, &format!("{path}::Crate"), None, pos).unwrap();

    assert_eq!(*scene.world.name(group).unwrap(), "Crate");
    assert_eq!(scene.world.transform(group).unwrap().position, pos);
    assert!(
        scene.world.mesh(group).is_none(),
        "the group is an empty pivot"
    );
    // Ordered by level, not file order; the children are the renderers.
    assert_eq!(
        level_names(&scene, group),
        vec![vec!["Cube.000"], vec!["Cube.001"]]
    );
    let g = scene.world.lod_group(group).unwrap().clone();
    assert_eq!(g.size, 1.0, "LOD0's largest extent");
    assert!(g.levels[0].screen_height > g.levels[1].screen_height);
    for r in g.renderers() {
        assert_eq!(scene.world.parent_id(r), Some(group));
        assert_eq!(scene.world.transform(r).unwrap().position, Vec3::ZERO);
        assert!(scene.world.mesh(r).is_some());
    }
    // A name that is neither a sub-object nor a set still errors.
    assert!(instantiate_asset(&mut scene, &format!("{path}::Lamp"), None, pos).is_err());
}

#[test]
fn the_whole_file_groups_sets_and_spawns_the_rest_alone() {
    let path = write_gltf("rusty_472_model.gltf");
    let asset = crate::asset::import_and_sync_sidecar(std::path::Path::new(&path)).unwrap();
    let mut scene = Scene::new();
    let ids = instantiate_model(&mut scene, &path, &asset, Vec3::ZERO);

    let names: Vec<String> = ids
        .iter()
        .map(|&id| scene.world.name(id).unwrap().clone())
        .collect();
    assert_eq!(names, vec!["Crate", "Barrel"]);
    assert!(scene.world.has_lod_group(ids[0]));
    assert!(!scene.world.has_lod_group(ids[1]));
}

#[test]
fn renderer_references_follow_a_prefab_stamp() {
    let path = write_gltf("rusty_472_prefab.gltf");
    let mut scene = Scene::new();
    let group = instantiate_asset(&mut scene, &format!("{path}::Crate"), None, Vec3::ZERO).unwrap();
    let prefab = extract_prefab(&scene, group).unwrap();

    let copy = instantiate_prefab(&mut scene, &prefab, None);
    assert_ne!(copy, group);
    assert_eq!(
        level_names(&scene, copy),
        vec![vec!["Cube.000"], vec!["Cube.001"]]
    );
    let g = scene.world.lod_group(copy).unwrap().clone();
    for r in g.renderers() {
        assert_eq!(
            scene.world.parent_id(r),
            Some(copy),
            "the copy's own children"
        );
    }
}
