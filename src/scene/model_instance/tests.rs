//! A whole model spawns as its node tree (#1083): a root named after the file,
//! one child per node at its local transform, an empty node as an empty child,
//! and a `_LOD<n>` set as one LODGroup where its first member sits.

use glam::{Quat, Vec3};

use super::instantiate_model;
use crate::scene::Scene;

/// A kit: an empty `Kit` node moved 10 m along X, holding a `Wall` turned a
/// quarter about Y and a crate at two levels of detail, each level's node at its
/// own height. Every mesh is one shared triangle.
pub(super) const KIT: &str = r#"{
  "asset": { "version": "2.0" },
  "scene": 0,
  "scenes": [ { "nodes": [ 0 ] } ],
  "nodes": [
    { "name": "Kit", "translation": [10.0, 0.0, 0.0], "children": [ 1, 2, 3 ] },
    { "name": "Wall", "mesh": 0, "translation": [0.0, 0.0, 5.0],
      "rotation": [0.0, 0.70710677, 0.0, 0.70710677] },
    { "name": "Crate_LOD0", "mesh": 1, "translation": [0.0, 2.0, 0.0] },
    { "name": "Crate_LOD1", "mesh": 2, "translation": [0.0, 3.0, 0.0] }
  ],
  "meshes": [
    { "name": "WallMesh", "primitives": [ { "attributes": { "POSITION": 0 } } ] },
    { "name": "Cube.000", "primitives": [ { "attributes": { "POSITION": 0 } } ] },
    { "name": "Cube.001", "primitives": [ { "attributes": { "POSITION": 0 } } ] }
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

fn spawn(position: Vec3) -> (Scene, u32) {
    spawn_file("rusty_1083_kit.gltf", KIT, position)
}

/// Write `gltf` as `file` in the test folder and spawn it whole at `position`.
pub(super) fn spawn_file(file: &str, gltf: &str, position: Vec3) -> (Scene, u32) {
    let path = crate::test_temp::dir().join(file);
    std::fs::write(&path, gltf).unwrap();
    let path = path.to_string_lossy().into_owned();
    let asset = crate::asset::import_and_sync_sidecar(std::path::Path::new(&path)).unwrap();
    let mut scene = Scene::new();
    let root = instantiate_model(&mut scene, &path, &asset, position);
    (scene, root)
}

pub(super) fn child(scene: &Scene, parent: u32, name: &str) -> u32 {
    let named = |&id: &u32| scene.world.name(id).is_some_and(|n| *n == name);
    let found = scene.world.children(parent).into_iter().find(named);
    found.unwrap_or_else(|| panic!("no child {name}"))
}

#[test]
fn every_node_is_a_child_at_its_local_transform() {
    let at = Vec3::new(1.0, 0.0, -2.0);
    let (scene, root) = spawn(at);
    assert_eq!(*scene.world.name(root).unwrap(), "rusty_1083_kit");
    assert_eq!(scene.world.transform(root).unwrap().position, at);
    assert!(scene.world.mesh(root).is_none());

    let kit = child(&scene, root, "Kit");
    assert!(
        scene.world.mesh(kit).is_none(),
        "an empty node is an empty child"
    );
    assert_eq!(scene.world.transform(kit).unwrap().position, Vec3::X * 10.0);

    let wall = child(&scene, kit, "Wall");
    assert!(scene.world.mesh(wall).is_some());
    let t = scene.world.transform(wall).unwrap();
    let quarter = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    assert!(t.rotation.abs_diff_eq(quarter, 1e-6));
    let world = scene.world.world_position(wall);
    assert!(
        world.abs_diff_eq(at + Vec3::new(10.0, 0.0, 5.0), 1e-5),
        "{world}"
    );
}

#[test]
fn a_lod_set_is_one_group_where_its_first_member_sits() {
    let (scene, root) = spawn(Vec3::ZERO);
    let kit = child(&scene, root, "Kit");
    assert_eq!(scene.world.children(kit).len(), 2, "Wall and the group");
    let group = child(&scene, kit, "Crate");
    assert_eq!(
        scene.world.transform(group).unwrap().position,
        Vec3::Y * 2.0
    );
    let levels = scene.world.lod_group(group).unwrap().levels.clone();
    let height = |l: usize| scene.world.world_position(levels[l].renderers[0]).y;
    assert!((height(0) - 2.0).abs() < 1e-5, "LOD0 at its node");
    assert!((height(1) - 3.0).abs() < 1e-5, "LOD1 at its own node");
}

#[test]
fn a_scale_factor_scales_the_node_offsets_with_the_meshes() {
    let path = crate::test_temp::dir().join("rusty_1083_scaled.gltf");
    std::fs::write(&path, KIT).unwrap();
    std::fs::write(path.with_extension("gltf.meta"), r#"{ "scale": 0.5 }"#).unwrap();
    let asset = crate::asset::import_file(&path).unwrap();
    let kit = asset.nodes.iter().find(|n| n.name == "Kit").unwrap();
    assert_eq!(kit.translation, Vec3::X * 5.0, "10 m at half scale");
    assert_eq!(
        kit.scale,
        Vec3::ONE,
        "the factor scales offsets, not node scales"
    );
}
