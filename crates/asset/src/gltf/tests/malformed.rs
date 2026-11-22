//! A malformed file: what its readers could not use is dropped with a warning, or the
//! file is refused.

use super::import::TRIANGLES;
use crate::gltf::import::import_slice;
use rusty_test_support::models::anim::{animated_buffer, ANIMATED_GLTF};
use rusty_test_support::models::{glb, quad_mode_gltf};

/// [`ANIMATED_GLTF`] as a `.glb`, edited first.
pub(super) fn animated_glb(edit: impl FnOnce(&mut serde_json::Value)) -> Vec<u8> {
    let mut doc: serde_json::Value = serde_json::from_str(ANIMATED_GLTF).unwrap();
    doc["buffers"][0].as_object_mut().unwrap().remove("uri");
    edit(&mut doc);
    glb(&doc.to_string(), &animated_buffer())
}

#[test]
fn a_channel_with_fewer_values_than_key_times_is_dropped() {
    rusty_test_support::logs::init();
    // Two key times, one output value: the sampler would index past the values.
    let asset = import_slice(&animated_glb(|d| d["accessors"][5]["count"] = 1.into())).unwrap();
    let sub = &asset.sub_meshes[0];
    assert!(sub.skin.is_some(), "the skin is kept");
    assert!(
        sub.clips.is_empty(),
        "the clip's one channel is dropped, and the clip with it"
    );
    let warned = rusty_test_support::logs::take();
    assert!(
        warned
            .iter()
            .any(|w| w.contains("'Wave'") && w.contains("'Joint0'")),
        "the warning names the animation and the node: {warned:?}"
    );
    assert_eq!(asset.validate(), Ok(()));
}

#[test]
fn a_primitive_naming_a_vertex_it_lacks_is_dropped() {
    let (json, bin) = quad_mode_gltf(TRIANGLES, &[0, 1, 2, 0, 2, 9]);
    let mut doc: serde_json::Value = serde_json::from_str(&json).unwrap();
    doc["buffers"][0].as_object_mut().unwrap().remove("uri");
    let asset = import_slice(&glb(&doc.to_string(), &bin)).unwrap();
    assert!(asset.sub_meshes[0].indices.is_empty());
    assert_eq!(asset.validate(), Ok(()));
}

#[test]
fn an_influence_on_a_joint_the_skin_lacks_is_dropped() {
    let mut bin = animated_buffer();
    bin[36 + 4] = 7; // the second vertex's first joint: the skin has two
    let mut doc: serde_json::Value = serde_json::from_str(ANIMATED_GLTF).unwrap();
    doc["buffers"][0].as_object_mut().unwrap().remove("uri");
    let asset = import_slice(&glb(&doc.to_string(), &bin)).unwrap();
    let v = &asset.sub_meshes[0].vertices[1];
    assert_eq!((v.joint_indices[0], v.joint_weights[0]), (0, 0.0));
    assert_eq!(asset.validate(), Ok(()));
}

#[test]
fn an_accessor_of_the_wrong_shape_for_its_use_refuses_the_file() {
    // The gltf crate's reader would take a VEC2 `JOINTS_0` as VEC4 and panic.
    let bytes = animated_glb(|d| d["accessors"][1]["type"] = "VEC2".into());
    let err = import_slice(&bytes).unwrap_err().to_string();
    assert!(
        err.contains("accessor 1") && err.contains("Joints"),
        "{err}"
    );
}

#[test]
fn a_position_naming_no_accessor_refuses_the_file() {
    // The gltf crate's own validation would index past its accessors and panic.
    let bytes =
        animated_glb(|d| d["meshes"][0]["primitives"][0]["attributes"]["POSITION"] = 9.into());
    let err = import_slice(&bytes).unwrap_err().to_string();
    assert!(err.contains("names accessor 9"), "{err}");
}

#[test]
fn a_glb_declaring_a_length_under_its_header_is_refused() {
    // The gltf crate subtracts the header's 12 bytes from it and overflows.
    let mut bytes = animated_glb(|_| {});
    bytes[8..12].copy_from_slice(&4u32.to_le_bytes());
    assert!(import_slice(&bytes).is_err());
}

#[test]
fn a_channel_targeting_an_unknown_property_or_no_node_is_refused() {
    let path =
        animated_glb(|d| d["animations"][0]["channels"][0]["target"]["path"] = "tint".into());
    assert!(import_slice(&path)
        .unwrap_err()
        .to_string()
        .contains("no known property"));
    let node = animated_glb(|d| d["animations"][0]["channels"][0]["target"]["node"] = 9.into());
    assert!(import_slice(&node)
        .unwrap_err()
        .to_string()
        .contains("targets no node"));
}

#[test]
fn a_node_hierarchy_that_is_not_a_forest_is_refused() {
    let cycle = animated_glb(|d| d["nodes"][2]["children"] = serde_json::json!([1]));
    assert!(import_slice(&cycle)
        .unwrap_err()
        .to_string()
        .contains("its own ancestor"));
    let shared = animated_glb(|d| d["nodes"][0]["children"] = serde_json::json!([2]));
    assert!(import_slice(&shared).is_err(), "a node with two parents");
}
