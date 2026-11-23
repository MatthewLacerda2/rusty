//! The packed specular image (the level in alpha, the colour in RGB) is typed by the pair of
//! fields as they stand after each change, whatever the order they were set in.

use super::*;
use rusty_asset::sidecar::texture;
use std::path::PathBuf;

fn image(name: &str) -> (PathBuf, String) {
    let png = rusty_test_support::temp::dir().join(name);
    image::RgbaImage::new(4, 4).save(&png).unwrap();
    let path = png.to_string_lossy().into_owned();
    (png, path)
}

fn library() -> MaterialLibrary {
    let mut lib = MaterialLibrary::new();
    lib.insert("m".into(), MaterialAsset::default());
    lib
}

#[test]
fn the_packed_image_stays_srgb_whichever_map_is_set_first() {
    let (level_first, level_path) = image("level_first_2302.png");
    let mut lib = library();
    set_specular_map(&mut lib, "m", Some(level_path.clone()));
    assert!(!texture(&level_first).srgb, "the level map alone is data");
    set_specular_color_map(&mut lib, "m", Some(level_path));
    assert!(texture(&level_first).srgb, "level, then colour: sRGB");

    let (colour_first, colour_path) = image("colour_first_2302.png");
    let mut lib = library();
    set_specular_color_map(&mut lib, "m", Some(colour_path.clone()));
    set_specular_map(&mut lib, "m", Some(colour_path));
    assert!(texture(&colour_first).srgb, "colour, then level: sRGB");
}

#[test]
fn un_setting_one_map_types_the_image_for_the_one_left() {
    let (png, path) = image("unset_2302.png");
    let mut lib = library();
    set_specular_map(&mut lib, "m", Some(path.clone()));
    set_specular_color_map(&mut lib, "m", Some(path.clone()));
    set_specular_color_map(&mut lib, "m", None);
    assert!(!texture(&png).srgb, "the level map left: data again");
    set_specular_color_map(&mut lib, "m", Some(path));
    assert!(texture(&png).srgb, "packed again: colour");
    set_specular_map(&mut lib, "m", Some(String::new()));
    assert!(texture(&png).srgb, "the colour map left: colour");
}

#[test]
fn a_level_map_beside_another_colour_map_is_data() {
    let (level, level_path) = image("apart_level_2302.png");
    let (tint, tint_path) = image("apart_tint_2302.png");
    let mut lib = library();
    set_specular_color_map(&mut lib, "m", Some(tint_path));
    set_specular_map(&mut lib, "m", Some(level_path));
    assert!(!texture(&level).srgb && texture(&tint).srgb);
}
