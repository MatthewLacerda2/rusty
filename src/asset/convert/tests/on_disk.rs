//! A named image is a reference only when the conversion wrote it (#1072): one
//! that lies outside the cache's folder is a file on disk, and stays its path,
//! whichever converter named it.

use crate::asset::convert::glb::{self, Document, Material, Texture};
use crate::asset::gltf::import::import_from;
use crate::asset::MapRef;

/// A `.glb` in `<dir>/cache/` whose two materials sample a named image: one in
/// the cache's textures folder, one in `<dir>/art/`.
fn write(dir: &std::path::Path) -> std::path::PathBuf {
    let textures = dir.join("cache").join("m.glb.0.textures");
    std::fs::create_dir_all(&textures).unwrap();
    std::fs::create_dir_all(dir.join("art")).unwrap();
    std::fs::write(textures.join("skin.png"), b"\x89PNG").unwrap();
    std::fs::write(dir.join("art").join("bricks.png"), b"\x89PNG").unwrap();
    let material = |name: &str, uri: &str, image: &str| Material {
        name: name.into(),
        base_color_texture: Some(Texture {
            uri: uri.into(),
            name: Some(image.into()),
        }),
        ..Material::default()
    };
    let doc = Document {
        materials: vec![
            material("Made", "m.glb.0.textures/skin.png", "Skin"),
            material("OnDisk", "../art/bricks.png", "Bricks"),
        ],
        ..Document::default()
    };
    let out = dir.join("cache").join("m.glb");
    std::fs::write(&out, glb::write(&doc)).unwrap();
    out
}

#[test]
fn only_an_image_inside_the_cache_folder_becomes_a_reference() {
    let dir = crate::test_temp::dir();
    let asset = import_from(&write(&dir), Some("m.blend")).expect("imports");
    let map = |name: &str| {
        let m = asset.materials.iter().find(|m| m.name == name).expect(name);
        m.base_color_map
            .as_ref()
            .map(MapRef::as_str)
            .expect("a map")
            .to_string()
    };
    assert_eq!(map("Made"), "m.blend#tex/Skin");
    let on_disk = std::path::PathBuf::from(map("OnDisk"));
    assert_eq!(on_disk, dir.join("art").join("bricks.png"));
}
