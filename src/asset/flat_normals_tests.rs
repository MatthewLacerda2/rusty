//! Each importer, given a mesh with no normals, shades it flat (#1217): every
//! corner's normal is its triangle's face normal, never a straight-up default.

use super::import_file;
use super::mesh_data::SubMesh;
use glam::Vec3;

/// Every triangle of `mesh` has three unit normals on the side it winds
/// counter-clockwise on, perpendicular to its plane.
pub(crate) fn assert_flat(mesh: &SubMesh) {
    assert!(!mesh.indices.is_empty(), "{} has no triangles", mesh.id);
    for tri in mesh.indices.chunks(3) {
        let [a, b, c] = [0, 1, 2].map(|i| mesh.vertices[tri[i] as usize]);
        let [pa, pb, pc] = [a, b, c].map(|v| Vec3::from(v.position));
        let face = (pb - pa).cross(pc - pa).normalize();
        for v in [a, b, c] {
            let n = Vec3::from(v.normal);
            assert!(n.dot(face) > 1.0 - 1e-5, "{tri:?}: normal {n}, face {face}");
        }
    }
}

/// Two triangles folded along their shared edge 1-2: one faces +Z, one +Y.
const FOLDED_OBJ: &str = "\
o Fold
v 0.0 0.0 0.0
v 1.0 0.0 0.0
v 0.0 1.0 0.0
v 0.0 0.0 -1.0
f 1 2 3
f 1 2 4
";

#[test]
fn an_obj_without_vn_lines_is_shaded_flat() {
    let path = crate::test_temp::dir().join("rusty_flat_fold.obj");
    std::fs::write(&path, FOLDED_OBJ).unwrap();
    let fold = import_file(&path).unwrap().sub_meshes.remove(0);
    assert_flat(&fold);
    // The shared corners split: each face keeps its own copy.
    assert_eq!(fold.vertices.len(), 6);
    assert_eq!(fold.vertices[0].normal, [0.0, 0.0, 1.0]);
    assert_eq!(fold.vertices[3].normal, [0.0, 1.0, 0.0]);
}

#[test]
fn a_gltf_primitive_without_normal_is_shaded_flat() {
    let path = crate::test_temp::dir().join("rusty_flat_tri.gltf");
    std::fs::write(&path, super::fixtures::TRIANGLE_GLTF).unwrap();
    let tri = import_file(&path).unwrap().sub_meshes.remove(0);
    assert_flat(&tri);
    assert_eq!(tri.vertices[0].normal, [0.0, 0.0, 1.0]);
}

#[test]
fn a_gltf_with_normals_keeps_them() {
    let dir = crate::test_temp::dir().join("rusty_flat_kept");
    std::fs::create_dir_all(&dir).unwrap();
    // One triangle in z=0 whose authored normals all lean +X: kept, not recomputed.
    let positions = [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
    let normals = [1.0f32, 0.0, 0.0].repeat(3);
    let bin: Vec<u8> = positions
        .iter()
        .chain(&normals)
        .flat_map(|f| f.to_le_bytes())
        .collect();
    std::fs::write(dir.join("tri.bin"), bin).unwrap();
    let path = dir.join("tri.gltf");
    std::fs::write(&path, LEANING_GLTF).unwrap();
    let tri = import_file(&path).unwrap().sub_meshes.remove(0);
    assert_eq!((tri.vertices.len(), tri.indices.len()), (3, 3));
    assert!(tri.vertices.iter().all(|v| v.normal == [1.0, 0.0, 0.0]));
}

/// A non-indexed triangle with `POSITION` and `NORMAL`, in a sibling `tri.bin`.
const LEANING_GLTF: &str = r#"{
  "asset": { "version": "2.0" },
  "scene": 0, "scenes": [ { "nodes": [ 0 ] } ], "nodes": [ { "mesh": 0 } ],
  "meshes": [ { "name": "Tri",
    "primitives": [ { "attributes": { "POSITION": 0, "NORMAL": 1 } } ] } ],
  "accessors": [
    { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
      "min": [0.0, 0.0, 0.0], "max": [1.0, 1.0, 0.0] },
    { "bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3" }
  ],
  "bufferViews": [
    { "buffer": 0, "byteOffset": 0, "byteLength": 36 },
    { "buffer": 0, "byteOffset": 36, "byteLength": 36 }
  ],
  "buffers": [ { "byteLength": 72, "uri": "tri.bin" } ]
}"#;
