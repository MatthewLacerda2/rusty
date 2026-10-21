//! Embedded FBX textures extracted beside the cached `.glb` (#1082).

use super::super::convert;
use crate::asset::gltf;
use crate::asset::ImportedAsset;
use std::path::{Path, PathBuf};

const PNG: &[u8] = b"\x89PNG";

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/models/embedded_textures.fbx")
}

/// The fixture converted into `dir`, then imported.
fn import(dir: &Path) -> ImportedAsset {
    let out = dir.join("quad.fbx.glb");
    convert(&fixture(), &out).expect("the fixture converts");
    gltf::import::import(&out).expect("its glTF imports")
}

fn material<'a>(asset: &'a ImportedAsset, name: &str) -> &'a crate::asset::MaterialData {
    asset.materials.iter().find(|m| m.name == name).expect(name)
}

#[test]
fn an_embedded_texture_is_extracted_beside_the_glb() {
    let dir = crate::test_temp::dir();
    let asset = import(&dir);
    let red = material(&asset, "Red")
        .base_color_map
        .clone()
        .expect("a map");
    assert_eq!(Path::new(&red), dir.join("quad.fbx.glb.textures/skin.png"));
    assert!(std::fs::read(red).unwrap().starts_with(PNG));
}

#[test]
fn a_clashing_name_climbing_out_of_its_folder_lands_beside_the_first() {
    let dir = crate::test_temp::dir();
    let asset = import(&dir);
    let blue = material(&asset, "Blue")
        .base_color_map
        .clone()
        .expect("a map");
    assert_eq!(
        Path::new(&blue),
        dir.join("quad.fbx.glb.textures/Skin.1.png")
    );
    let [red, blue] = ["skin.png", "Skin.1.png"]
        .map(|n| std::fs::read(dir.join("quad.fbx.glb.textures").join(n)).unwrap());
    assert!(blue.starts_with(PNG));
    assert_ne!(red, blue, "the second texture overwrote nothing");
}

#[test]
fn an_embedded_format_the_importer_cannot_read_is_skipped() {
    let asset = import(&crate::test_temp::dir());
    assert_eq!(material(&asset, "Blue").normal_map, None);
}

#[test]
fn a_reconversion_removes_files_it_no_longer_writes() {
    let dir = crate::test_temp::dir();
    let textures = dir.join("quad.fbx.glb.textures");
    std::fs::create_dir_all(&textures).unwrap();
    std::fs::write(textures.join("stale.png"), b"old").unwrap();
    import(&dir);
    let mut names: Vec<String> = std::fs::read_dir(&textures)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["Skin.1.png", "skin.png"]);
}

#[test]
fn names_are_sanitised_and_kept_apart_ignoring_case() {
    let taken = vec!["skin.png".to_string()];
    assert_eq!(super::unique("Skin", "png", &taken), "Skin.1.png");
    assert_eq!(super::unique("other", "png", &taken), "other.png");
}
