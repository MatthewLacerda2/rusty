//! The node walk (#1083): parents first, names filled in, a skinned mesh's joints
//! left out and what one holds hung from its bone.

use glam::{Quat, Vec3};

use super::model_nodes;

/// An armature whose one joint carries a mesh and holds a sword, the skinned body
/// beside it, an unnamed mesh node, an unnamed empty one, and a joint of a skin
/// no mesh uses.
const RIG: &str = r#"{
  "asset": { "version": "2.0" },
  "scene": 0,
  "scenes": [ { "nodes": [ 0, 4, 5, 6 ] } ],
  "nodes": [
    { "name": "Armature", "translation": [5.0, 0.0, 0.0], "children": [ 1, 3 ] },
    { "name": "Hip", "mesh": 0, "translation": [0.0, 1.0, 0.0], "children": [ 2 ] },
    { "name": "Sword", "translation": [0.0, 0.0, 1.0] },
    { "name": "Body", "mesh": 0, "skin": 0 },
    { "mesh": 0, "rotation": [0.0, 0.70710677, 0.0, 0.70710677] },
    { },
    { "name": "Loose", "translation": [0.0, 7.0, 0.0] }
  ],
  "skins": [ { "joints": [ 1 ] }, { "joints": [ 6 ] } ],
  "meshes": [ { "name": "Tri", "primitives": [ { "attributes": { "POSITION": 0 } } ] } ],
  "accessors": [
    { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
      "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 0.0] }
  ],
  "bufferViews": [ { "buffer": 0, "byteOffset": 0, "byteLength": 36 } ],
  "buffers": [ { "byteLength": 36 } ]
}"#;

#[test]
fn nodes_come_parents_first_and_bones_hold_theirs() {
    let gltf = gltf::Gltf::from_slice(RIG.as_bytes()).unwrap();
    let nodes = model_nodes(&gltf.document, &["Tri"]);
    let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
    assert_eq!(
        names,
        ["Armature", "Body", "Tri", "node_5", "Loose", "Hip", "Sword"]
    );
    let parents: Vec<Option<usize>> = nodes.iter().map(|n| n.parent).collect();
    assert_eq!(parents, [None, Some(0), None, None, None, Some(1), Some(1)]);
    assert_eq!(nodes[0].translation, Vec3::new(5.0, 0.0, 0.0));
    assert_eq!(nodes[1].mesh.as_deref(), Some("Tri"));
    assert_eq!(
        nodes[2].mesh.as_deref(),
        Some("Tri"),
        "an unnamed node takes its mesh's name"
    );
    let quarter = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    assert!(nodes[2].rotation.abs_diff_eq(quarter, 1e-6));
    assert_eq!(nodes[3].mesh, None);
    assert!(nodes[4].bone.is_none(), "a joint no mesh skins is a node");
    assert_eq!(nodes[4].translation, Vec3::Y * 7.0);
}

#[test]
fn a_joint_mesh_and_a_held_child_hang_from_the_bone() {
    let gltf = gltf::Gltf::from_slice(RIG.as_bytes()).unwrap();
    let nodes = model_nodes(&gltf.document, &["Tri"]);
    let (hip, sword) = (&nodes[5], &nodes[6]);
    for held in [hip, sword] {
        let bone = held.bone.as_ref().expect("held by a bone");
        assert_eq!(bone.name, "Hip");
        let rest = bone.rest.transform_point3(Vec3::ZERO);
        assert_eq!(rest, Vec3::Y, "the bone's rest relative to Body");
    }
    assert_eq!(hip.mesh.as_deref(), Some("Tri"), "the joint's mesh is kept");
    assert_eq!(hip.translation, Vec3::ZERO, "at the bone itself");
    assert_eq!(sword.translation, Vec3::Z, "its own offset from the bone");
}

#[test]
fn a_bone_rest_is_relative_to_a_moved_skinned_node() {
    // Body 3 m along Z and turned a quarter about Y, beside the armature.
    let mut gltf: serde_json::Value = serde_json::from_str(RIG).unwrap();
    gltf["nodes"][3]["translation"] = serde_json::json!([0.0, 0.0, 3.0]);
    gltf["nodes"][3]["rotation"] = serde_json::json!([0.0, 0.70710677, 0.0, 0.70710677]);
    let gltf = gltf::Gltf::from_slice(gltf.to_string().as_bytes()).unwrap();
    let nodes = model_nodes(&gltf.document, &["Tri"]);
    let rest = nodes[6].bone.as_ref().unwrap().rest;
    let worlds = crate::asset::model_node::worlds(&nodes);
    // Sword sits at Armature (5,0,0) + Hip (0,1,0) + its own (0,0,1).
    let sword = worlds[6].transform_point3(Vec3::ZERO);
    assert!(sword.abs_diff_eq(Vec3::new(5.0, 1.0, 1.0), 1e-5), "{sword}");
    let body = worlds[1];
    let hip = (body * rest).transform_point3(Vec3::ZERO);
    assert!(hip.abs_diff_eq(Vec3::new(5.0, 1.0, 0.0), 1e-5), "{hip}");
}
