//! The watch re-converts only what a change touched (#1327): a source it names, or
//! one whose conversion read a file it names; never every source.

use super::super::gate::{refresh_changed, touched};
use super::super::{cache_location, cached_gltf, record};
use crate::core::fs::written::Stale;
use crate::core::paths::native;
use std::path::{Path, PathBuf};

/// A folder holding a model whose texture is missing (its conversion read that
/// path) and one whose textures are embedded, both converted.
fn converted() -> (PathBuf, PathBuf, PathBuf) {
    let dir = crate::test_temp::dir();
    let fixtures = &native("tests/fixtures/models");
    let missing = dir.join("missing.fbx");
    let embedded = dir.join("embedded.fbx");
    std::fs::copy(fixtures.join("missing_texture.fbx"), &missing).unwrap();
    std::fs::copy(fixtures.join("embedded_textures.fbx"), &embedded).unwrap();
    cached_gltf(&missing).unwrap();
    cached_gltf(&embedded).unwrap();
    (dir, missing, embedded)
}

fn noted(paths: &[&Path]) -> Stale {
    Stale::new(paths.iter().map(|p| p.to_string_lossy().into()).collect())
}

#[test]
fn a_noted_source_or_a_file_its_conversion_read_is_touched() {
    let (dir, missing, embedded) = converted();
    let out = cache_location(&missing).unwrap();
    let stored = record::stored(&out).unwrap();
    let texture = stored.depends_on[0].path().to_path_buf();
    assert_eq!(touched(&dir, &noted(&[&texture])), vec![missing.clone()]);
    assert_eq!(touched(&dir, &noted(&[&embedded])), vec![embedded]);
    let elsewhere = dir.join("unrelated.png");
    assert!(touched(&dir, &noted(&[&elsewhere])).is_empty());
    assert!(touched(&dir, &noted(&[])).is_empty());
}

#[test]
fn a_failed_source_is_not_retried_when_something_else_changes() {
    let (dir, missing, embedded) = converted();
    std::fs::write(&missing, "not an fbx").unwrap();
    assert!(touched(&dir, &noted(&[&missing])).contains(&missing));
    assert_eq!(refresh_changed(&dir, vec![]), 0);
    let other = vec![embedded.to_string_lossy().into_owned()];
    assert!(!touched(&dir, &noted(&[&embedded])).contains(&missing));
    assert_eq!(refresh_changed(&dir, other), 1);
}
