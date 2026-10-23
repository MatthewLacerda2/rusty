//! Extracted texture names, and the URIs a source file's own name leads to (#1082).

use super::super::{file_name, unique};
use super::{convert_and_import, fixture, material, textures, PNG};
use crate::asset::MapRef;

fn name(raw: &str) -> String {
    let (stem, ext) = file_name(raw, b"");
    format!("{stem}.{ext}")
}

#[test]
fn a_file_name_keeps_no_separator_climb_or_escape() {
    assert_eq!(name("textures/sub/skin.PNG"), "skin.png");
    assert_eq!(name(r"C:\art\..\Skin.png"), "Skin.png");
    assert_eq!(name("../../etc/passwd.png"), "passwd.png");
    assert_eq!(name("..png"), "texture.png");
    assert_eq!(name("100%25 red#2.png"), "100_25_red_2.png");
    assert_eq!(name("..."), "texture.");
    assert_eq!(name(""), "texture.");
}

#[test]
fn a_name_without_an_image_extension_takes_its_bytes_format() {
    let png = b"\x89PNG\r\n\x1a\nrest";
    let jpeg = [0xFF, 0xD8, 0xFF, 0xE0];
    let named = |raw: &str, bytes: &[u8]| file_name(raw, bytes);
    assert_eq!(
        named("Material #25", png),
        ("Material__25".into(), "png".into())
    );
    assert_eq!(named("scan.bmp", &jpeg), ("scan.bmp".into(), "jpg".into()));
    assert_eq!(named("...", png), ("texture".into(), "png".into()));
    assert_eq!(
        named("skin.tga", png),
        ("skin".into(), "tga".into()),
        "an image name stays"
    );
}

#[test]
fn names_are_sanitised_and_kept_apart_ignoring_case() {
    let taken = vec!["skin.png".to_string()];
    assert_eq!(unique("Skin", "png", &taken), "Skin.1.png");
    assert_eq!(unique("other", "png", &taken), "other.png");
}

#[test]
fn a_source_name_a_uri_would_misread_still_finds_its_textures() {
    // A `:` can't be in a Windows file name (Windows isn't shipped yet), so only
    // the other names run there.
    let colon = (!cfg!(windows)).then_some("Soldier: v2.fbx");
    for source in colon.into_iter().chain(["bad%C0.fbx", "lit%20eral.fbx"]) {
        let dir = crate::test_temp::dir();
        let copy = dir.join(source);
        std::fs::copy(fixture("embedded_textures.fbx"), &copy).unwrap();
        let out = dir.join(format!("{source}.glb"));
        let asset = convert_and_import(&copy, &out);
        let red = material(&asset, "Red")
            .base_color_map
            .as_ref()
            .and_then(MapRef::file)
            .expect(source);
        let expected = textures(&out).join("skin.png");
        assert_eq!(red, expected, "{source}");
        assert!(std::fs::read(&red).unwrap().starts_with(PNG), "{source}");
    }
}
