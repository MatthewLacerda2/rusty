//! The node walk (#1083): parents first, names filled in, joints left out and
//! what hangs under one composed onto the node above it.

use glam::{Quat, Vec3};

use super::model_nodes;

/// An armature whose one joint holds a sword, the skinned body beside it, an
/// unnamed mesh node and an unnamed empty one.
const RIG: &str = r#"{
  "asset": { "version": "2.0" },
  "scene": 0,
  "scenes": [ { "nodes": [ 0, 4, 5 ] } ],
  "nodes": [
    { "name": "Armature", "translation": [5.0, 0.0, 0.0], "children": [ 1, 3 ] },
    { "name": "Hip", "translation": [0.0, 1.0, 0.0], "children": [ 2 ] },
    { "name": "Sword", "translation": [0.0, 0.0, 1.0] },
    { "name": "Body", "mesh": 0, "skin": 0 },
    { "mesh": 0, "rotation": [0.0, 0.70710677, 0.0, 0.70710677] },
    { }
  ],
  "skins": [ { "joints": [ 1 ] } ],
  "meshes": [ { "name": "Tri", "primitives": [ { "attributes": { "POSITION": 0 } } ] } ],
  "accessors": [
    { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
      "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 0.0] }
  ],
  "bufferViews": [ { "buffer": 0, "byteOffset": 0, "byteLength": 36 } ],
  "buffers": [ { "byteLength": 36 } ]
}"#;

#[test]
fn nodes_come_parents_first_without_the_joints() {
    let gltf = gltf::Gltf::from_slice(RIG.as_bytes()).unwrap();
    let nodes = model_nodes(&gltf.document, &["Tri"]);
    let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(names, ["Armature", "Sword", "Body", "Tri", "node_5"]);
    let parents: Vec<Option<usize>> = nodes.iter().map(|n| n.parent).collect();
    assert_eq!(parents, [None, Some(0), Some(0), None, None]);
    assert_eq!(nodes[0].translation, Vec3::new(5.0, 0.0, 0.0));
    // The joint's offset is carried onto what it held.
    assert!(nodes[1]
        .translation
        .abs_diff_eq(Vec3::new(0.0, 1.0, 1.0), 1e-6));
    assert_eq!(nodes[2].mesh.as_deref(), Some("Tri"));
    assert_eq!(
        nodes[3].mesh.as_deref(),
        Some("Tri"),
        "an unnamed node takes its mesh's name"
    );
    let quarter = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    assert!(nodes[3].rotation.abs_diff_eq(quarter, 1e-6));
    assert_eq!(nodes[4].mesh, None);
}
