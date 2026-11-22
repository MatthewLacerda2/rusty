use image::RgbaImage;

use super::*;
use crate::sidecar::{set_texture, texture};

fn shrunk_warnings(source: &Path) -> usize {
    let named = format!("[Import] {}: ", source.display());
    rusty_test_support::logs::take()
        .iter()
        .filter(|m| m.starts_with(&named) && m.contains("larger than its Max Size"))
        .count()
}

fn reimport(source: &Path) {
    let settings = texture(source);
    refresh(source, "").unwrap();
    for compress in [false, true] {
        load(source, &settings, TextureKind::Colour, compress).unwrap();
    }
}

#[test]
fn a_file_larger_than_the_default_warns_once_across_reimports() {
    rusty_test_support::logs::init();
    let dir = rusty_test_support::temp::dir();
    let wide = dir.join("wide.png");
    RgbaImage::new(8192, 4).save(&wide).unwrap();
    reimport(&wide);
    assert_eq!(shrunk_warnings(&wide), 1, "the first import says so");
    reimport(&wide);
    assert_eq!(shrunk_warnings(&wide), 0, "and no re-import again");
}

#[test]
fn a_chosen_2048_is_kept_by_a_bake_and_warns_once() {
    rusty_test_support::logs::init();
    let dir = rusty_test_support::temp::dir();
    let wall = dir.join("wall.png");
    RgbaImage::new(4096, 4).save(&wall).unwrap();
    let meta = crate::sidecar::meta_path(&wall);
    let chosen = r#"{"texture": {"max_size": 2048, "max_size_chosen": true}}"#;
    std::fs::write(&meta, chosen).unwrap();
    crate::sidecar::prepare_bake("test", (4096, 4), [wall.to_str().unwrap()]).unwrap();
    assert_eq!(
        imported_size(&wall, &texture(&wall)),
        Some(((4096, 4), (2048, 4)))
    );
    reimport(&wall);
    reimport(&wall);
    assert_eq!(shrunk_warnings(&wall), 1);
    let raised = TextureSettings {
        max_size: 4096,
        ..texture(&wall)
    };
    set_texture(&wall, raised).unwrap();
    reimport(&wall);
    assert_eq!(shrunk_warnings(&wall), 0, "raised to fit, it is quiet");
}

#[test]
fn a_file_that_fits_never_warns() {
    rusty_test_support::logs::init();
    let dir = rusty_test_support::temp::dir();
    let tile = dir.join("tile.png");
    RgbaImage::new(2048, 8).save(&tile).unwrap();
    reimport(&tile);
    assert_eq!(shrunk_warnings(&tile), 0);
}

#[test]
fn a_file_no_max_size_holds_is_told_to_be_saved_smaller() {
    rusty_test_support::logs::init();
    let dir = rusty_test_support::temp::dir();
    let long = dir.join("long.png");
    RgbaImage::new(16384, 4).save(&long).unwrap();
    shrink::warn_if_shrunk(&long, (16384, 4), 2048);
    let logged = rusty_test_support::logs::take();
    let advice = logged.iter().find(|m| m.contains("long.png")).unwrap();
    assert!(advice.contains("save the file smaller"), "{advice}");
    assert!(!advice.contains("raise Max Size"), "{advice}");
}
