//! A mesh with several materials imports as ONE sub-mesh whose index list is split
//! into material runs (#1285), from every source format.

use super::{import_file, ImportedAsset, SubMesh};

/// `sub`'s runs as `(indices, material name)`.
fn runs(asset: &ImportedAsset, sub: &SubMesh) -> Vec<(std::ops::Range<u32>, String)> {
    let name = |m: Option<usize>| m.map_or("-".into(), |i| asset.materials[i].name.clone());
    sub.ranges
        .iter()
        .map(|r| (r.indices.clone(), name(r.material)))
        .collect()
}

/// The FBX fixture's quad maps its two triangles `ByPolygon` to `Red` and `Blue`:
/// the converter writes two primitives, which come back as two runs of one quad.
#[test]
fn an_fbx_quad_with_two_materials_is_one_sub_mesh_with_two_runs() {
    let path = crate::test_temp::dir().join("quad.fbx");
    std::fs::write(
        &path,
        include_str!("../../tests/fixtures/models/embedded_textures.fbx"),
    )
    .unwrap();
    let asset = import_file(&path).unwrap();
    assert_eq!(asset.sub_mesh_ids(), ["Quad"]);
    let quad = asset.sub_mesh("Quad").unwrap();
    assert_eq!(
        runs(&asset, quad),
        [(0..3, "Red".to_string()), (3..6, "Blue".to_string())]
    );
}

const OBJ: &str = "mtllib quad.mtl
o Quad
v 0 0 0
v 1 0 0
v 1 1 0
v 0 1 0
vn 0 0 1
usemtl Red
f 1//1 2//1 3//1
usemtl Blue
f 1//1 3//1 4//1
";

const MTL: &str = "newmtl Red\nKd 1 0 0\nnewmtl Blue\nKd 0 0 1\n";

/// `tobj` splits an object at each `usemtl` into models sharing its name; they are
/// joined back into one sub-mesh, so `quad.obj::Quad` names the whole object.
#[test]
fn an_obj_object_split_by_usemtl_is_one_sub_mesh_with_two_runs() {
    let dir = crate::test_temp::dir();
    std::fs::write(dir.join("quad.obj"), OBJ).unwrap();
    std::fs::write(dir.join("quad.mtl"), MTL).unwrap();
    let asset = import_file(&dir.join("quad.obj")).unwrap();
    assert_eq!(asset.sub_mesh_ids(), ["Quad"]);
    let quad = asset.sub_mesh("Quad").unwrap();
    assert_eq!(quad.indices.len(), 6);
    assert_eq!(
        runs(&asset, quad),
        [(0..3, "Red".to_string()), (3..6, "Blue".to_string())]
    );
}
