use std::path::{Path, PathBuf};

use super::super::{fit_data_slot, meta_path, set_texture, texture, TextureSettings};
use super::*;

fn prepare(path: &Path, side: u32) -> Result<(), String> {
    prepare_bake("test", (side, 4), [path.to_str().unwrap()])
}

fn png(dir: &Path, name: &str, w: u32) -> PathBuf {
    let path = dir.join(name);
    image::RgbaImage::new(w, 4).save(&path).unwrap();
    path
}

#[test]
fn a_bake_with_no_sidecar_ships_at_its_size_and_one_that_fits_writes_none() {
    let dir = rusty_test_support::temp::dir();
    for (side, kept) in [(4096, 4096), (3000, 4096), (8192, 8192)] {
        // Not on disk yet: the sidecar is written before the PNG.
        let baked = dir.join(format!("wall_{side}.png"));
        prepare(&baked, side).unwrap();
        assert_eq!(texture(&baked).max_size, kept, "a {side} bake");
    }
    let small = dir.join("small.png");
    prepare(&small, 2048).unwrap();
    assert!(!meta_path(&small).exists(), "a bake that fits writes none");
}

#[test]
fn an_engine_max_size_follows_every_rebake() {
    let dir = rusty_test_support::temp::dir();
    let path = dir.join("wall.png");
    for (side, kept) in [(4096, 4096), (8192, 8192), (1024, 2048), (4096, 4096)] {
        prepare(&path, side).unwrap();
        assert_eq!(texture(&path).max_size, kept, "baked at {side}");
        assert!(!texture(&path).max_size_chosen, "still the engine's");
    }
}

#[test]
fn a_slot_fixs_sidecar_and_a_sparse_hand_written_one_are_the_engines() {
    let dir = rusty_test_support::temp::dir();
    let rough = png(&dir, "rough.png", 256);
    fit_data_slot(&rough);
    assert!(meta_path(&rough).exists() && !texture(&rough).srgb);
    // A sparse block without the key, as a sidecar written before it existed.
    let hand = png(&dir, "hand.png", 256);
    std::fs::write(meta_path(&hand), r#"{"texture": {"max_size": 1024}}"#).unwrap();
    for path in [&rough, &hand] {
        prepare(path, 4096).unwrap();
        assert_eq!(texture(path).max_size, 4096, "{}", path.display());
    }
    assert!(!texture(&rough).srgb, "the rest is kept");
}

#[test]
fn a_developers_max_size_is_never_changed_by_a_bake() {
    let dir = rusty_test_support::temp::dir();
    let set = png(&dir, "set.png", 4096);
    let chosen = TextureSettings::default().chosen(1024);
    set_texture(&set, chosen).unwrap();
    // A developer who picks the default itself picks it too.
    let default = png(&dir, "default.png", 4096);
    set_texture(&default, TextureSettings::default().chosen(2048)).unwrap();
    let hand = png(&dir, "hand.png", 4096);
    let text = r#"{"texture": {"max_size": 2048, "max_size_chosen": true}}"#;
    std::fs::write(meta_path(&hand), text).unwrap();
    for (path, kept) in [(&set, 1024), (&default, 2048), (&hand, 2048)] {
        prepare(path, 4096).unwrap();
        assert_eq!(texture(path).max_size, kept, "{}", path.display());
    }
    assert_eq!(texture(&set), chosen, "nothing else changes either");
}

#[test]
fn the_choice_is_stored_only_when_made() {
    let dir = rusty_test_support::temp::dir();
    let path = png(&dir, "a.png", 4);
    set_texture(
        &path,
        TextureSettings {
            srgb: false,
            ..Default::default()
        },
    )
    .unwrap();
    let text = std::fs::read_to_string(meta_path(&path)).unwrap();
    assert!(!text.contains("max_size_chosen"), "{text}");
    set_texture(&path, TextureSettings::default().chosen(512)).unwrap();
    let text = std::fs::read_to_string(meta_path(&path)).unwrap();
    assert!(text.contains("\"max_size_chosen\": true"), "{text}");
}

#[test]
fn nothing_is_written_during_play_or_in_the_import_cache() {
    let dir = rusty_test_support::temp::dir();
    let playing = dir.join("playing.png");
    let hold = crate::convert::PlayHold::take();
    prepare(&playing, 4096).unwrap();
    drop(hold);
    assert!(!meta_path(&playing).exists(), "Play writes no sidecar");
    let cache = dir.join(crate::convert::IMPORT_CACHE_DIR);
    std::fs::create_dir_all(&cache).unwrap();
    let cached = cache.join("albedo.png");
    prepare(&cached, 4096).unwrap();
    assert!(
        !meta_path(&cached).exists(),
        "the import cache is left alone"
    );
}

#[test]
fn a_bake_above_the_largest_max_size_is_refused_before_anything_is_written() {
    let dir = rusty_test_support::temp::dir();
    let tall = dir.join("tall.png");
    assert_eq!(
        prepare_bake("t", (4, 8192), [tall.to_str().unwrap()]),
        Ok(())
    );
    assert_eq!(texture(&tall).max_size, 8192, "either side counts");
    let huge = dir.join("huge.png");
    let err = prepare_bake("t", (16384, 64), [huge.to_str().unwrap()]).unwrap_err();
    assert!(err.contains("16384x64") && err.contains("8192"), "{err}");
    assert!(!meta_path(&huge).exists(), "refused before its sidecar");
}
