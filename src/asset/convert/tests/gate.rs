//! Play never converts (#1253): with conversion off, a changed source is served
//! from the cache as it stands and converted once conversion is back on, and a
//! source never imported fails its load.

use super::super::{allow_conversion, cached_gltf, convert_held};
use crate::asset::import_file;
use std::path::{Path, PathBuf};

/// A quad (6 indices) under the object name `Quad`.
const QUAD: &str = include_str!("../../../../tests/fixtures/models/embedded_textures.fbx");

/// One triangle (3 indices) under the same object name.
fn triangle() -> String {
    include_str!("../../../../tests/fixtures/models/missing_texture.fbx").replace("Tri", "Quad")
}

fn source(text: &str) -> PathBuf {
    let path = crate::test_temp::dir().join("model.fbx");
    std::fs::write(&path, text).unwrap();
    path
}

fn indices(source: &Path) -> usize {
    let asset = import_file(source).unwrap();
    asset.sub_mesh("Quad").unwrap().indices.len()
}

fn stamp(cache: &Path) -> String {
    let mut name = cache.as_os_str().to_owned();
    name.push(".source");
    std::fs::read_to_string(PathBuf::from(name)).unwrap()
}

#[test]
fn with_conversion_off_a_changed_source_is_served_from_its_cache() {
    let path = source(QUAD);
    let cache = cached_gltf(&path).unwrap();
    let (before, glb) = (stamp(&cache), std::fs::read(&cache).unwrap());

    allow_conversion(false);
    std::fs::write(&path, triangle()).unwrap();
    assert_eq!(cached_gltf(&path).unwrap(), cache);
    assert_eq!(indices(&path), 6, "every load reads the last import");
    assert_eq!(indices(&path), 6);
    assert_eq!(stamp(&cache), before, "no conversion ran");
    assert_eq!(std::fs::read(&cache).unwrap(), glb);
    assert_eq!(convert_held(), 0, "nothing converts while it is off");
    assert_eq!(stamp(&cache), before);

    allow_conversion(true);
    assert_eq!(convert_held(), 1, "the held source converts once");
    assert_ne!(stamp(&cache), before);
    assert_eq!(indices(&path), 3, "the new import");
    assert_eq!(convert_held(), 0, "and is no longer held");
}

#[test]
fn with_conversion_off_a_current_source_is_not_held() {
    let path = source(QUAD);
    cached_gltf(&path).unwrap();
    allow_conversion(false);
    assert_eq!(indices(&path), 6);
    allow_conversion(true);
    assert_eq!(convert_held(), 0);
}

#[test]
fn with_conversion_off_a_source_never_imported_fails_its_load() {
    let path = source(QUAD);
    allow_conversion(false);
    let err = cached_gltf(&path).unwrap_err().to_string();
    assert!(err.contains("import it in edit mode first"), "{err}");
    assert!(import_file(&path).is_err(), "the load fails, no panic");
    allow_conversion(true);
    assert_eq!(convert_held(), 0, "a missing cache is not held");
    assert_eq!(indices(&path), 6, "edit mode imports it");
}
