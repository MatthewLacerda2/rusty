//! The one loader of an authored module (#1638): a build that may write re-bakes a
//! stale module on disk, a shipped player re-composes it in memory and writes nothing.

use std::path::PathBuf;

use super::load_against;
use crate::shadergen::bake_recipe;
use crate::shadergen::rebake::tests::{copy_engine, engine_copy, rim};
use crate::shadergen::validate::validate;

/// A `rim` bake against one engine copy, and a second, changed copy it is stale against.
fn stale(tag: &str) -> (PathBuf, PathBuf, String) {
    let (root, engine) = engine_copy(tag);
    let out = root.join("out");
    let wgsl = PathBuf::from(bake_recipe(&rim(), &engine, out.to_str().unwrap()).unwrap());
    let changed = copy_engine(&root.join("engine_v2"));
    let common = PathBuf::from(&changed).join("common.wgsl");
    let text = std::fs::read_to_string(&common).unwrap() + "\n// changed\n";
    std::fs::write(&common, text).unwrap();
    (root, wgsl, changed)
}

#[test]
fn a_player_re_composes_a_stale_module_in_memory_and_writes_nothing() {
    let (root, wgsl, engine) = stale("player");
    let before = std::fs::read_to_string(&wgsl).unwrap();
    let baked = load_against(&wgsl, &engine, false).unwrap();
    assert_ne!(baked.source, before, "the up-to-date module");
    validate(&engine, &baked.source).expect("composes");
    assert!(baked.layout(&wgsl).is_ok() && baked.params.is_some());
    assert_eq!(
        std::fs::read_to_string(&wgsl).unwrap(),
        before,
        "files as shipped"
    );
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn an_editor_re_bakes_a_stale_module_on_disk_and_loads_it() {
    let (root, wgsl, engine) = stale("editor");
    let baked = load_against(&wgsl, &engine, true).unwrap();
    assert_eq!(
        std::fs::read_to_string(&wgsl).unwrap(),
        baked.source,
        "re-baked"
    );
    validate(&engine, &baked.source).expect("composes");
    // Up to date, a player loads exactly what is on disk.
    assert_eq!(load_against(&wgsl, &engine, false).unwrap(), baked);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn a_missing_module_is_an_error_and_a_bare_one_loads_as_it_is() {
    let (root, engine) = engine_copy("bare_load");
    let wgsl = root.join("bare.wgsl");
    assert!(load_against(&wgsl, &engine, false).is_err());
    std::fs::write(&wgsl, "// hand-written\n").unwrap();
    let baked = load_against(&wgsl, &engine, false).unwrap();
    assert_eq!(
        (baked.source.as_str(), baked.params),
        ("// hand-written\n", None)
    );
    std::fs::remove_dir_all(&root).ok();
}
