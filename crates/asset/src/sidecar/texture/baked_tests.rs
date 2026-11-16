//! A bake moves an engine-chosen Max Size to its size and never a developer's
//! (`keep_baked_size`, #2259); a bake above the largest Max Size is refused.

use std::path::{Path, PathBuf};

use super::super::{fit_data_slot, meta_path, set_texture, texture, TextureSettings};
use super::*;

/// A `w`-wide, 4-tall PNG at `dir/name`, thin so a large side costs little.
fn png(dir: &Path, name: &str, w: u32) -> PathBuf {
    let path = dir.join(name);
    image::RgbaImage::new(w, 4).save(&path).unwrap();
    path
}

#[test]
fn a_bake_with_no_sidecar_ships_at_its_size_and_one_that_fits_writes_none() {
    let dir = rusty_test_support::temp::dir();
    for (side, kept) in [(4096, 4096), (3000, 4096), (8192, 8192)] {
        let baked = png(&dir, &format!("wall_{side}.png"), side);
        assert_eq!(keep_baked_size(&baked), Ok(true));
        assert_eq!(texture(&baked).max_size, kept, "a {side} bake");
    }
    let small = png(&dir, "small.png", 2048);
    assert_eq!(keep_baked_size(&small), Ok(false));
    assert!(!meta_path(&small).exists(), "a bake that fits writes none");
}

#[test]
fn an_engine_max_size_follows_every_rebake() {
    let dir = rusty_test_support::temp::dir();
    let path = png(&dir, "wall.png", 4096);
    keep_baked_size(&path).unwrap();
    for (side, kept) in [(8192, 8192), (1024, 2048), (4096, 4096)] {
        png(&dir, "wall.png", side);
        keep_baked_size(&path).unwrap();
        assert_eq!(texture(&path).max_size, kept, "rebaked at {side}");
    }
}

#[test]
fn a_slot_fixs_default_sidecar_is_raised_and_keeps_the_rest() {
    let dir = rusty_test_support::temp::dir();
    let path = png(&dir, "rough.png", 256);
    fit_data_slot(&path);
    assert!(meta_path(&path).exists() && !texture(&path).srgb);
    png(&dir, "rough.png", 4096);
    assert_eq!(keep_baked_size(&path), Ok(true));
    let s = texture(&path);
    assert_eq!((s.max_size, s.srgb), (4096, false));
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
    std::fs::write(meta_path(&hand), r#"{"texture": {"max_size": 2048}}"#).unwrap();
    for (path, kept) in [(&set, 1024), (&default, 2048), (&hand, 2048)] {
        assert_eq!(keep_baked_size(path), Ok(false), "{}", path.display());
        assert_eq!(texture(path).max_size, kept, "{}", path.display());
    }
    assert_eq!(texture(&set), chosen, "nothing else changes either");
}

#[test]
fn nothing_is_written_during_play_or_in_the_import_cache() {
    let dir = rusty_test_support::temp::dir();
    let playing = png(&dir, "playing.png", 4096);
    let hold = crate::convert::PlayHold::take();
    assert_eq!(keep_baked_size(&playing), Ok(false));
    drop(hold);
    assert!(!meta_path(&playing).exists(), "Play writes no sidecar");
    let cache = dir.join(crate::convert::IMPORT_CACHE_DIR);
    std::fs::create_dir_all(&cache).unwrap();
    let cached = png(&cache, "albedo.png", 4096);
    assert_eq!(keep_baked_size(&cached), Ok(false));
    assert!(
        !meta_path(&cached).exists(),
        "the import cache is left alone"
    );
}

#[test]
fn a_bake_above_the_largest_max_size_is_refused() {
    assert_eq!(check_bake_size(8192, 4), Ok(()));
    assert_eq!(check_bake_size(4, 8192), Ok(()));
    let err = check_bake_size(16384, 64).unwrap_err();
    assert!(err.contains("16384x64") && err.contains("8192"), "{err}");
}
