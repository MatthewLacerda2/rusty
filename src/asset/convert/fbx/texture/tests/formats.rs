//! Embedded textures the `gltf` crate can't decode, or whose names don't say their
//! format, still import with their mesh (#1082).

use super::{convert_and_import, fixture, material, textures};
use crate::asset::ImportedAsset;
use std::path::Path;

fn import(dir: &Path) -> ImportedAsset {
    convert_and_import(
        &fixture("embedded_odd_textures.fbx"),
        &dir.join("odd.fbx.glb"),
    )
}

/// The bytes the map `uri` names, which must be `dir`'s extracted `file`.
fn extracted(dir: &Path, uri: Option<String>, file: &str) -> Vec<u8> {
    let path = textures(&dir.join("odd.fbx.glb")).join(file);
    assert_eq!(uri.as_deref().map(Path::new), Some(path.as_path()));
    std::fs::read(path).unwrap()
}

#[test]
fn an_embedded_tga_imports_with_its_mesh() {
    let dir = crate::test_temp::dir();
    let asset = import(&dir);
    assert!(!asset.sub_meshes.is_empty(), "the mesh survived");
    let red = material(&asset, "Red").base_color_map.clone();
    assert_eq!(
        extracted(&dir, red, "body_d.tga")[2],
        2,
        "an uncompressed TGA"
    );
}

#[test]
fn bytes_that_mismatch_their_name_import_with_their_mesh() {
    let dir = crate::test_temp::dir();
    let asset = import(&dir);
    assert!(!asset.sub_meshes.is_empty(), "the mesh survived");
    let blue = material(&asset, "Blue").base_color_map.clone();
    assert!(extracted(&dir, blue, "skin.png").starts_with(&[0xFF, 0xD8, 0xFF]));
}

#[test]
fn a_texture_named_only_by_its_element_takes_its_bytes_format() {
    let dir = crate::test_temp::dir();
    let asset = import(&dir);
    let normal = material(&asset, "Red").normal_map.clone();
    assert!(extracted(&dir, normal, "Material__25.png").starts_with(b"\x89PNG"));
}
