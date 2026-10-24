//! The import cache (#1024): where a converted source is cached, and that it is
//! re-made exactly when the source changes.

use super::{converter_for, is_import_cache, refresh, Converted, Converter, Staging};
use crate::asset::ImportError;
use crate::core::paths::native;
use std::path::Path;

mod changed;
mod depends;
mod failures;
mod folder_tag;
mod gate;
mod identity;
mod locking;
mod looks;
mod notes;
mod on_disk;
mod refresh;
mod skips;
mod staging;
mod tool;
mod unlocked;

/// A converter that copies its source, so a test can see which bytes it wrote.
const COPY: Converter = Converter {
    extension: "src",
    name: "copy",
    version: 1,
    tool: None,
    convert: copy,
};

fn copy(source: &Path, to: &Staging) -> Result<Converted, ImportError> {
    std::fs::copy(source, &to.glb)
        .map(|_| Converted::default())
        .map_err(|e| ImportError::Parse(e.to_string()))
}

#[test]
fn the_cache_folder_is_recognised_and_ordinary_folders_are_not() {
    assert!(is_import_cache(&native(
        "assets/generated/imported/models/a.fbx.glb"
    )));
    assert!(!is_import_cache(&native("assets/generated/voice.mp3")));
    assert!(!is_import_cache(&native("assets/models/a.glb")));
}

#[test]
fn fbx_is_converted_and_gltf_is_not() {
    assert!(converter_for(&native("a/B.FBX")).is_some());
    assert!(converter_for(&native("a/b.glb")).is_none());
}

#[test]
fn the_cache_is_remade_only_when_the_source_changes() {
    let dir = crate::test_temp::dir().join("convert_refresh");
    std::fs::create_dir_all(&dir).unwrap();
    let (source, out) = (dir.join("a.src"), dir.join("cache").join("a.src.glb"));
    std::fs::write(&source, "one").unwrap();

    refresh(&COPY, &source, &out).unwrap();
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "one");

    // Up to date: a second refresh leaves the cached file alone.
    std::fs::write(&out, "kept").unwrap();
    refresh(&COPY, &source, &out).unwrap();
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "kept");

    std::fs::write(&source, "two").unwrap();
    refresh(&COPY, &source, &out).unwrap();
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "two");
}

#[test]
fn a_new_converter_version_remakes_the_cache() {
    let dir = crate::test_temp::dir().join("convert_version");
    std::fs::create_dir_all(&dir).unwrap();
    let (source, out) = (dir.join("a.src"), dir.join("a.src.glb"));
    std::fs::write(&source, "one").unwrap();
    refresh(&COPY, &source, &out).unwrap();
    std::fs::write(&out, "stale").unwrap();

    let newer = Converter { version: 2, ..COPY };
    refresh(&newer, &source, &out).unwrap();
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "one");
}

#[test]
fn a_missing_source_keeps_its_cache_and_without_one_is_an_error() {
    let dir = crate::test_temp::dir().join("convert_shipped");
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("gone.src.glb");
    assert!(refresh(&COPY, &dir.join("gone.src"), &out).is_err());

    std::fs::write(&out, "shipped").unwrap();
    refresh(&COPY, &dir.join("gone.src"), &out).unwrap();
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "shipped");
}

#[test]
fn the_cache_is_a_folder_of_the_projects_generated_assets() {
    let generated = crate::core::project::GENERATED_DIR;
    assert_eq!(super::IMPORT_CACHE_DIR, format!("{generated}/imported"));
}
