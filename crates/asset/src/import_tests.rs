//! `import_all` (#2319): it makes an image's compressed copy where a headless
//! load would draw it decoded, and writes nothing while a world plays;
//! `import_strict` stops on a broken model before encoding any image.

use super::{import_all, import_strict};
use crate::convert::PlayHold;
use crate::sidecar::TextureSettings;
use crate::texture_import::{cache_file, load, Texels, TextureKind};
use std::path::{Path, PathBuf};

/// A fresh folder holding one 16² PNG, its default settings asking for BC7.
fn folder(name: &str) -> (PathBuf, PathBuf) {
    let root = rusty_test_support::temp::dir().join(name);
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let png = root.join("albedo.png");
    image::RgbaImage::from_pixel(16, 16, image::Rgba([200, 100, 50, 255]))
        .save(&png)
        .unwrap();
    // Outside a project the cache sits in the system temp dir, keyed by path: a
    // reused pid could find a dead run's.
    let _ = std::fs::remove_file(cache_file(&png).unwrap());
    (root, png)
}

fn compressed(png: &Path) -> bool {
    let loaded = load(png, &TextureSettings::default(), TextureKind::Colour, true);
    matches!(loaded.unwrap().texels, Texels::Blocks(_))
}

#[test]
fn an_import_makes_the_cache_a_load_then_reads() {
    let (root, png) = folder("import_all_makes");
    assert!(!compressed(&png), "no cache yet: decoded");
    let done = import_all(&root).unwrap();
    assert_eq!(done.textures.encoded, 1);
    assert!(done.sources.is_empty() && done.failed.is_empty());
    assert!(cache_file(&png).unwrap().is_file());
    assert!(compressed(&png), "read from the cache");
    assert_eq!(
        import_all(&root).unwrap().textures.encoded,
        0,
        "current: no re-encode"
    );
}

#[test]
fn an_import_during_play_is_refused_and_writes_nothing() {
    let (root, png) = folder("import_all_refused");
    let hold = PlayHold::take();
    let refused = import_all(&root).unwrap_err();
    assert!(refused.contains("stop first"), "{refused}");
    assert!(!cache_file(&png).unwrap().is_file());
    drop(hold);
    assert_eq!(import_all(&root).unwrap().textures.encoded, 1);
}

#[test]
fn a_broken_model_stops_the_strict_import_before_any_image() {
    let (root, png) = folder("import_strict_broken");
    std::fs::write(root.join("broken.fbx"), b"not an fbx").unwrap();
    let refused = import_strict(&root).unwrap_err();
    assert!(refused.contains("broken.fbx"), "{refused}");
    assert!(!cache_file(&png).unwrap().is_file(), "no image encoded");
    let done = import_all(&root).unwrap();
    assert_eq!(done.failed.len(), 1, "listed, and the rest still ran");
    assert_eq!(done.textures.encoded, 1);
}

#[test]
fn an_import_cooks_every_model_once() {
    let model = crate::cook::fixture::model("import_all_cooks");
    let root = model.parent().unwrap();
    let run = || crate::cook::with_writes(true, || import_all(root).unwrap().cooked);
    assert_eq!(run(), 1, "the model is cooked");
    assert!(crate::cook::cooked_path(&model).unwrap().is_file());
    assert_eq!(run(), 0, "its cooked file is current");
}
