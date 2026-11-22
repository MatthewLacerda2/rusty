//! Topology normalization: every triangle mode imports as the same list; non-surface
//! modes are skipped. A malformed file drops what its readers could not use.

use crate::gltf::import::import_slice;
use crate::import_file;
use crate::mesh_data::SubMesh;
use rusty_test_support::models::anim::{animated_buffer, ANIMATED_GLTF};
use rusty_test_support::models::{glb, quad_mode_gltf};

// glTF 2.0's `primitive.mode` values.
const POINTS: u32 = 0;
const LINES: u32 = 1;
const LINE_LOOP: u32 = 2;
const LINE_STRIP: u32 = 3;
const TRIANGLES: u32 = 4;
const TRIANGLE_STRIP: u32 = 5;
const TRIANGLE_FAN: u32 = 6;

fn import_quad(name: &str, mode: u32, indices: &[u16]) -> SubMesh {
    let dir = rusty_test_support::temp::dir().join(format!("rusty_mode_{name}"));
    std::fs::create_dir_all(&dir).unwrap();
    let (json, bin) = quad_mode_gltf(mode, indices);
    std::fs::write(dir.join("quad.bin"), bin).unwrap();
    let path = dir.join("quad.gltf");
    std::fs::write(&path, json).unwrap();
    let mut asset = import_file(&path).unwrap();
    assert_eq!(asset.sub_mesh_ids(), vec!["Quad".to_string()]);
    asset.sub_meshes.remove(0)
}

#[test]
fn triangles_mode_passes_through() {
    let quad = import_quad("tris", TRIANGLES, &[0, 1, 2, 0, 2, 3]);
    assert_eq!(quad.vertices.len(), 4);
    assert_eq!(quad.indices, vec![0, 1, 2, 0, 2, 3]);
}

#[test]
fn triangle_strip_matches_equivalent_triangle_list() {
    // Odd strip triangles swap their leading pair to keep the winding.
    let strip = import_quad("strip", TRIANGLE_STRIP, &[0, 1, 3, 2]);
    let list = import_quad("strip_ref", TRIANGLES, &[0, 1, 3, 3, 1, 2]);
    assert_eq!(strip.indices, list.indices);
    assert_eq!(strip.vertices, list.vertices);
}

#[test]
fn triangle_fan_matches_equivalent_triangle_list() {
    let fan = import_quad("fan", TRIANGLE_FAN, &[0, 1, 2, 3]);
    let list = import_quad("fan_ref", TRIANGLES, &[0, 1, 2, 0, 2, 3]);
    assert_eq!(fan.indices, list.indices);
    assert_eq!(fan.vertices, list.vertices);
}

#[test]
fn non_indexed_strip_triangulates_the_sequential_run() {
    // No index accessor: the sequential run 0..4 must still go through the strip rewrite.
    let strip = import_quad("noidx_strip", TRIANGLE_STRIP, &[]);
    assert_eq!(strip.indices, vec![0, 1, 2, 2, 1, 3]);
}

#[test]
fn non_indexed_fan_triangulates_the_sequential_run() {
    let fan = import_quad("noidx_fan", TRIANGLE_FAN, &[]);
    assert_eq!(fan.indices, vec![0, 1, 2, 0, 2, 3]);
}

#[test]
fn non_surface_modes_are_skipped() {
    for (name, mode) in [
        ("points", POINTS),
        ("lines", LINES),
        ("line_loop", LINE_LOOP),
        ("line_strip", LINE_STRIP),
    ] {
        let sub = import_quad(name, mode, &[0, 1, 2, 3]);
        assert!(sub.vertices.is_empty(), "{name}: vertices must be skipped");
        assert!(sub.indices.is_empty(), "{name}: indices must be skipped");
    }
}

/// [`ANIMATED_GLTF`] as a `.glb`, edited first.
fn animated_glb(edit: impl FnOnce(&mut serde_json::Value)) -> Vec<u8> {
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
