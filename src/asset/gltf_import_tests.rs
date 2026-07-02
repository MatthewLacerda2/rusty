//! src/asset/gltf_import_tests.rs — glTF topology normalization tests (#317):
//! every triangle primitive mode imports as the same triangle list, and the
//! non-surface modes are skipped instead of being misread as triangles.

use super::fixtures::quad_mode_gltf;
use super::import_file;
use super::mesh_data::SubMesh;

// glTF 2.0 `primitive.mode` values.
const POINTS: u32 = 0;
const LINES: u32 = 1;
const LINE_LOOP: u32 = 2;
const LINE_STRIP: u32 = 3;
const TRIANGLES: u32 = 4;
const TRIANGLE_STRIP: u32 = 5;
const TRIANGLE_FAN: u32 = 6;

/// Write the [`quad_mode_gltf`] fixture pair into its own temp dir and import
/// the one `Quad` sub-mesh.
fn import_quad(name: &str, mode: u32, indices: &[u16]) -> SubMesh {
    let dir = std::env::temp_dir().join(format!("rusty_mode_{name}"));
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
    // The strip [0,1,3,2] carries triangles {0,1,3} and {1,3,2}; the equivalent
    // triangle list spells them out (odd strip triangles swap their leading pair
    // to keep the winding).
    let strip = import_quad("strip", TRIANGLE_STRIP, &[0, 1, 3, 2]);
    let list = import_quad("strip_ref", TRIANGLES, &[0, 1, 3, 3, 1, 2]);
    assert_eq!(strip.indices, list.indices);
    assert_eq!(strip.vertices, list.vertices);
}

#[test]
fn triangle_fan_matches_equivalent_triangle_list() {
    // The fan [0,1,2,3] pivots on vertex 0: triangles (0,1,2) and (0,2,3) — the
    // exact index stream of the plain-TRIANGLES quad.
    let fan = import_quad("fan", TRIANGLE_FAN, &[0, 1, 2, 3]);
    let list = import_quad("fan_ref", TRIANGLES, &[0, 1, 2, 0, 2, 3]);
    assert_eq!(fan.indices, list.indices);
    assert_eq!(fan.vertices, list.vertices);
}

#[test]
fn non_indexed_strip_triangulates_the_sequential_run() {
    // No index accessor: the vertices themselves are in strip order, so the
    // sequential run 0..4 must go through the strip rewrite, not be misread as a
    // triangle list.
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
    // POINTS / LINES / LINE_LOOP / LINE_STRIP aren't surface meshes: the primitive
    // is skipped entirely (no vertices, no indices) instead of feeding a bogus
    // index stream to the triangle path.
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
