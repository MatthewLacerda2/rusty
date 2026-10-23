//! Embedded FBX textures extracted beside the cached `.glb` (#1082).

use super::super::convert_to;
use crate::asset::convert::textures_dirs;
use crate::asset::gltf;
use crate::asset::ImportedAsset;
use crate::asset::MapRef;
use std::path::{Path, PathBuf};

mod depends;
mod formats;
mod names;
mod skips;

const PNG: &[u8] = b"\x89PNG";

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/models")
        .join(name)
}

/// The fixture `source` converted to `out`, then imported.
fn convert_and_import(source: &Path, out: &Path) -> ImportedAsset {
    convert_to(source, out).expect("the fixture converts");
    gltf::import::import(out).expect("its glTF imports")
}

/// The one textures folder beside the cached glTF at `out`.
fn textures(out: &Path) -> PathBuf {
    let [dir] = textures_dirs(out).try_into().expect("one textures folder");
    dir
}

/// `embedded_textures.fbx` converted into `dir`, then imported.
fn import(dir: &Path) -> ImportedAsset {
    convert_and_import(&fixture("embedded_textures.fbx"), &dir.join("quad.fbx.glb"))
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
        .as_ref()
        .and_then(MapRef::file)
        .expect("a map");
    assert_eq!(
        red.as_path(),
        textures(&dir.join("quad.fbx.glb")).join("skin.png")
    );
    assert!(std::fs::read(red).unwrap().starts_with(PNG));
}

#[test]
fn a_clashing_name_climbing_out_of_its_folder_lands_beside_the_first() {
    let dir = crate::test_temp::dir();
    let asset = import(&dir);
    let blue = material(&asset, "Blue")
        .base_color_map
        .as_ref()
        .and_then(MapRef::file)
        .expect("a map");
    let textures = textures(&dir.join("quad.fbx.glb"));
    assert_eq!(blue, textures.join("Skin.1.png"));
    let [red, blue] = ["skin.png", "Skin.1.png"].map(|n| std::fs::read(textures.join(n)).unwrap());
    assert!(blue.starts_with(PNG));
    assert_ne!(red, blue, "the second texture overwrote nothing");
}

#[test]
fn an_embedded_format_the_importer_cannot_read_is_skipped() {
    let asset = import(&crate::test_temp::dir());
    assert_eq!(material(&asset, "Blue").normal_map, None);
}

#[test]
fn a_reconversion_replaces_an_older_converters_textures_folder() {
    let dir = crate::test_temp::dir();
    let stale = dir.join("quad.fbx.glb.textures");
    std::fs::create_dir_all(&stale).unwrap();
    std::fs::write(stale.join("stale.png"), b"old").unwrap();
    import(&dir);
    assert!(!stale.exists());
    let mut names: Vec<String> = std::fs::read_dir(textures(&dir.join("quad.fbx.glb")))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["Skin.1.png", "skin.png"]);
}

#[test]
fn a_material_with_only_a_texture_has_a_white_base_colour() {
    // The fixture's materials name no `DiffuseColor`: the texture alone colours
    // them, so the factor it multiplies is glTF's white, not black (#1280).
    let asset = import(&crate::test_temp::dir());
    assert_eq!(material(&asset, "Red").base_color, [1.0; 4]);
}
