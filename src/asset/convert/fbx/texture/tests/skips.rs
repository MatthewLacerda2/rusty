//! A texture left out on purpose is reported in `Converted::skipped` by the
//! converter itself (#1197), not only by `refresh` replaying it.

use super::fixture;
use crate::asset::convert::fbx::convert;
use crate::asset::convert::{Skip, Staging};
use crate::asset::{gltf, MapRef};
use std::path::PathBuf;

/// What converting the fixture `name` into a fresh folder skipped.
fn skipped(name: &str) -> Vec<Skip> {
    let dir = crate::test_temp::dir();
    let to = Staging {
        glb: dir.join("out.glb"),
        textures: dir.join("out.glb.textures"),
        out: dir.join("out.glb"),
    };
    convert(&fixture(name), &to)
        .expect("the fixture converts")
        .skipped
}

#[test]
fn a_texture_neither_on_disk_nor_embedded_is_skipped_by_name() {
    let skip = Skip {
        what: "FBX texture `Gone`".into(),
        why: "neither a file on disk nor embedded".into(),
    };
    assert_eq!(skipped("missing_texture.fbx"), [skip]);
}

#[test]
fn an_embedded_format_the_importer_cannot_read_is_skipped_by_name() {
    let [skip] = skipped("embedded_textures.fbx")
        .try_into()
        .expect("one skip");
    assert_eq!(skip.what, "FBX texture `Bumps`");
    assert!(skip.why.starts_with("embeds a `.dds`"), "{}", skip.why);
}

/// The fixture `name`, its texture file names rewritten `from` → `to`, written
/// into a fresh folder beside a file `to` holding `bytes`; then converted: what it
/// skipped, and the folder.
fn beside(name: &str, [from, to]: [&str; 2], bytes: &[u8]) -> (Vec<Skip>, PathBuf) {
    let dir = crate::test_temp::dir();
    let text = std::fs::read_to_string(fixture(name)).unwrap();
    std::fs::write(dir.join(name), text.replace(from, to)).unwrap();
    std::fs::write(dir.join(to), bytes).unwrap();
    let staging = Staging {
        glb: dir.join("out.glb"),
        textures: dir.join("out.glb.textures"),
        out: dir.join("out.glb"),
    };
    let converted = convert(&dir.join(name), &staging).expect("the fixture converts");
    (converted.skipped, dir)
}

#[test]
fn a_texture_on_disk_in_a_format_the_importer_cannot_read_is_skipped() {
    // Store packs ship `.psd` beside the FBX: referenced, it rendered flat white
    // (#1293). Skipped, the material keeps its colour.
    let (skips, dir) = beside("missing_texture.fbx", ["gone.png", "gone.psd"], b"8BPS");
    let [skip] = skips.try_into().expect("one skip");
    assert_eq!(skip.what, "FBX texture `Gone`");
    assert!(skip
        .why
        .ends_with("is a `.psd`, not an image format the importer reads"));
    let gone = gltf::import::import(&dir.join("out.glb"))
        .unwrap()
        .materials;
    assert_eq!(gone[0].base_color, [0.2, 0.4, 0.6, 1.0]);
}

#[test]
fn an_embedded_copy_stands_in_for_an_unreadable_format_on_disk() {
    let (skips, dir) = beside("embedded_textures.fbx", ["skin.png", "skin.psd"], b"8BPS");
    assert!(
        !skips.iter().any(|s| s.what == "FBX texture `Red`"),
        "{skips:?}"
    );
    let asset = gltf::import::import(&dir.join("out.glb")).unwrap();
    let red = asset.materials.iter().find(|m| m.name == "Red").unwrap();
    let map = red.base_color_map.as_ref().and_then(MapRef::file).unwrap();
    assert!(map.starts_with(dir.join("out.glb.textures")), "{map:?}");
}
