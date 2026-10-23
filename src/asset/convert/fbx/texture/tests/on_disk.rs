//! A texture on disk is referenced only when the renderer can load it: an image
//! extension, bytes that agree with it, not empty (#1293). Otherwise the next
//! file, then an embedded copy, stands in, or the texture is skipped and the
//! material keeps its colour.

use super::fixture;
use crate::asset::convert::fbx::convert;
use crate::asset::convert::{Skip, Staging};
use crate::asset::{gltf, MapRef, MaterialData};
use std::path::PathBuf;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";
const JPEG: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0];

/// The fixture `name` with each `(from, to)` rewrite (`{dir}` is the folder),
/// written into a fresh folder beside `files`, then converted and imported: what
/// it skipped, its materials, and the folder.
fn convert_in(
    name: &str,
    rewrites: &[(&str, &str)],
    files: &[(&str, &[u8])],
) -> (Vec<Skip>, Vec<MaterialData>, PathBuf) {
    let dir = crate::test_temp::dir();
    let mut text = std::fs::read_to_string(fixture(name)).unwrap();
    for (from, to) in rewrites {
        text = text.replace(from, &to.replace("{dir}", &dir.to_string_lossy()));
    }
    std::fs::write(dir.join(name), text).unwrap();
    for (file, bytes) in files {
        std::fs::write(dir.join(file), bytes).unwrap();
    }
    let out = dir.join("out.glb");
    let staging = Staging {
        glb: out.clone(),
        textures: dir.join("out.glb.textures"),
        out: out.clone(),
    };
    let skipped = convert(&dir.join(name), &staging)
        .expect("converts")
        .skipped;
    let materials = gltf::import::import(&out).expect("imports").materials;
    (skipped, materials, dir)
}

/// `missing_texture.fbx` beside one file, its texture renamed `file`: why `Gone`
/// was skipped, after checking its colour stayed.
fn gone_skipped_beside(file: &str, bytes: &[u8]) -> String {
    let (skips, materials, _) = convert_in(
        "missing_texture.fbx",
        &[("gone.png", file)],
        &[(file, bytes)],
    );
    assert_eq!(
        materials[0].base_color,
        [0.2, 0.4, 0.6, 1.0],
        "the colour stays"
    );
    assert_eq!(materials[0].base_color_map, None);
    let [skip] = skips.try_into().expect("one skip");
    assert_eq!(skip.what, "FBX texture `Gone`");
    skip.why
}

#[test]
fn a_format_the_importer_cannot_read_is_skipped_whatever_its_bytes() {
    // Store packs ship `.psd` beside the FBX: referenced, it rendered flat white.
    // The renderer decodes by extension, so PNG bytes inside don't save it.
    for bytes in [b"8BPS".as_slice(), PNG] {
        let why = gone_skipped_beside("gone.psd", bytes);
        assert!(why.ends_with("gone.psd is not an image format the importer reads"));
    }
    let why = gone_skipped_beside("gone", PNG);
    assert!(why.ends_with("gone is not an image format the importer reads"));
}

#[test]
fn bytes_that_disagree_with_the_name_are_skipped() {
    let why = gone_skipped_beside("gone.png", JPEG);
    assert!(
        why.ends_with("holds `.jpg` bytes under a `.png` name"),
        "{why}"
    );
}

#[test]
fn an_empty_file_is_skipped() {
    assert!(gone_skipped_beside("gone.png", b"").ends_with("gone.png is empty"));
}

#[test]
fn a_jpeg_named_jpeg_is_referenced() {
    let (skips, materials, dir) = convert_in(
        "missing_texture.fbx",
        &[("gone.png", "gone.jpeg")],
        &[("gone.jpeg", JPEG)],
    );
    assert_eq!(skips, []);
    assert_eq!(map(&materials[0]), dir.join("gone.jpeg"));
}

#[test]
fn a_usable_second_file_stands_in_for_an_unusable_first() {
    // The relative name finds a `.psd`; the absolute one, a PNG.
    let rewrites = [
        ("\"/missing/gone.png\"", "\"{dir}/real.png\""),
        ("gone.png", "gone.psd"),
    ];
    let files: [(&str, &[u8]); 2] = [("gone.psd", b"8BPS"), ("real.png", PNG)];
    let (skips, materials, dir) = convert_in("missing_texture.fbx", &rewrites, &files);
    assert_eq!(skips, []);
    assert_eq!(map(&materials[0]), dir.join("real.png"));
}

#[test]
fn an_embedded_copy_stands_in_for_an_unusable_file_on_disk() {
    let (skips, materials, dir) = convert_in(
        "embedded_textures.fbx",
        &[("skin.png", "skin.psd")],
        &[("skin.psd", b"8BPS")],
    );
    assert!(
        !skips.iter().any(|s| s.what == "FBX texture `Red`"),
        "{skips:?}"
    );
    let red = materials.iter().find(|m| m.name == "Red").unwrap();
    assert!(map(red).starts_with(dir.join("out.glb.textures")));
}

fn map(material: &MaterialData) -> PathBuf {
    let map = material.base_color_map.as_ref().and_then(MapRef::file);
    map.expect("a base colour map")
}
