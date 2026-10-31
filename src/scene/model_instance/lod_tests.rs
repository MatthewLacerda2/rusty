//! LODGroups of a whole-model spawn are grouped per node (#1083): an Alt+D copy
//! of a set on the same meshes is a group of its own, and a node that only
//! shares an LOD mesh is a plain child.

use glam::Vec3;

use super::tests::{child, spawn_file};

/// A shelf holding a crate at two levels, its Blender duplicate 5 m along X on
/// the same two meshes, and a `Lid` reusing the LOD0 mesh 5 m the other way.
const SHELF: &str = r#"{
  "asset": { "version": "2.0" },
  "scene": 0,
  "scenes": [ { "nodes": [ 0 ] } ],
  "nodes": [
    { "name": "Shelf", "children": [ 1, 2, 3, 4, 5 ] },
    { "name": "Crate_LOD0", "mesh": 0 },
    { "name": "Crate_LOD1", "mesh": 1, "translation": [0.0, 1.0, 0.0] },
    { "name": "Crate_LOD0.001", "mesh": 0, "translation": [5.0, 0.0, 0.0] },
    { "name": "Crate_LOD1.001", "mesh": 1, "translation": [5.0, 1.0, 0.0] },
    { "name": "Lid", "mesh": 0, "translation": [-5.0, 0.0, 0.0] }
  ],
  "meshes": [
    { "name": "Crate_LOD0", "primitives": [ { "attributes": { "POSITION": 0 } } ] },
    { "name": "Crate_LOD1", "primitives": [ { "attributes": { "POSITION": 0 } } ] }
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

#[test]
fn a_duplicated_set_is_a_group_of_its_own() {
    let (scene, root) = spawn_file("rusty_1083_shelf.gltf", SHELF, Vec3::ZERO);
    let shelf = child(&scene, root, "Shelf");
    assert_eq!(
        scene.world.children(shelf).len(),
        3,
        "two groups and the lid"
    );
    for (name, x) in [("Crate", 0.0), ("Crate.001", 5.0)] {
        let group = child(&scene, shelf, name);
        let levels = scene.world.lod_group(group).unwrap().levels.clone();
        assert_eq!(levels.len(), 2, "{name}");
        assert!(levels.iter().all(|l| l.renderers.len() == 1), "{name}");
        assert_eq!(scene.world.children(group).len(), 2, "{name}");
        let lod1 = scene.world.world_position(levels[1].renderers[0]);
        assert!(
            lod1.abs_diff_eq(Vec3::new(x, 1.0, 0.0), 1e-5),
            "{name}: {lod1}"
        );
    }
}

#[test]
fn a_node_sharing_an_lod_mesh_is_a_plain_child_in_its_place() {
    let (scene, root) = spawn_file("rusty_1083_shelf_lid.gltf", SHELF, Vec3::ZERO);
    let lid = child(&scene, child(&scene, root, "Shelf"), "Lid");
    assert!(scene.world.mesh(lid).is_some());
    assert!(scene.world.lod_group(lid).is_none());
    let at = scene.world.world_position(lid);
    assert!(at.abs_diff_eq(Vec3::new(-5.0, 0.0, 0.0), 1e-5), "{at}");
}
