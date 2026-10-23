//! References to extracted textures (#1130): what is one, and what it resolves to.

use super::{file, reference, source};
use crate::asset::import_file;
use std::path::{Path, PathBuf};

/// A copy of the embedded-textures fixture in a fresh folder, `text` edited by `edit`.
fn quad(edit: impl Fn(String) -> String) -> PathBuf {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/models/embedded_textures.fbx");
    let text = edit(std::fs::read_to_string(fixture).unwrap());
    let path = crate::test_temp::dir().join("quad.fbx");
    std::fs::write(&path, text).unwrap();
    path
}

fn base_color_map(path: &Path, material: &str) -> String {
    let asset = import_file(path).expect("imports");
    let data = asset.materials.iter().find(|m| m.name == material);
    data.and_then(|m| m.base_color_map.clone()).expect(material)
}

#[test]
fn a_plain_path_is_no_reference_and_names_itself() {
    for plain in [
        "skin.png",
        "a.png#tex/b",
        "model.glb#tex/Skin",
        "model.fbx#tex/",
    ] {
        assert_eq!(source(plain), None, "{plain}");
        assert_eq!(file(plain), Some(PathBuf::from(plain)), "{plain}");
    }
}

#[test]
fn a_reference_names_its_source() {
    let map = reference("assets/soldier.fbx", "Skin");
    assert_eq!(map, "assets/soldier.fbx#tex/Skin");
    assert_eq!(source(&map), Some("assets/soldier.fbx"));
}

#[test]
fn an_import_names_an_extracted_texture_by_reference_and_it_resolves() {
    let path = quad(|text| text);
    let source = path.to_string_lossy();
    let red = base_color_map(&path, "Red");
    assert_eq!(red, reference(&source, "Red"));
    let blue = base_color_map(&path, "Blue");
    assert_eq!(blue, reference(&source, "Blue"));
    let [red, blue] = [red, blue].map(|map| std::fs::read(file(&map).expect("resolves")).unwrap());
    assert!(red.starts_with(b"\x89PNG"));
    assert_ne!(red, blue, "each reference finds its own texture");
    assert_eq!(file(&reference(&source, "Green")), None);
}

#[test]
fn textures_sharing_a_name_are_named_by_index() {
    let path = quad(|text| text.replace("\"Texture::Blue\"", "\"Texture::Red\""));
    let source = path.to_string_lossy();
    assert_eq!(base_color_map(&path, "Red"), reference(&source, "#0"));
    assert_eq!(base_color_map(&path, "Blue"), reference(&source, "#1"));
}
