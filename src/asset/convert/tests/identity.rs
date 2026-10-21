//! A cache's identity is its whole source path (#1167): two sources never share
//! one, whatever their prefix, case or separators.

use super::super::{cache_path, project_relative};
use proptest::prelude::*;
use std::path::Path;

#[test]
fn a_project_caches_beside_its_generated_assets_under_the_whole_source_path() {
    let root = Path::new("/game");
    assert_eq!(
        cache_path(root, "assets/models/soldier.fbx"),
        Path::new("/game/assets/generated/imported/assets/models/soldier.fbx.glb")
    );
    assert_eq!(
        cache_path(root, "props/crate.FBX"),
        Path::new("/game/assets/generated/imported/props/crate.FBX.glb")
    );
}

/// The cache of the project-relative `parts`, in a project at `root`.
fn cache_of(root: &Path, parts: &[impl AsRef<str>]) -> std::path::PathBuf {
    let parts: Vec<&str> = parts.iter().map(AsRef::as_ref).collect();
    let rel = project_relative(root, Path::new(&parts.join("/"))).expect("in the project");
    cache_path(root, &rel)
}

/// A project folder, so `project_relative` takes its paths.
fn project() -> std::path::PathBuf {
    let root = crate::test_temp::dir();
    std::fs::write(root.join(crate::core::project::PROJECT_FILE), "").unwrap();
    root
}

#[test]
fn sources_differing_only_by_the_assets_prefix_get_distinct_caches() {
    let root = project();
    let with = cache_of(&root, &["assets", "props", "crate.fbx"]);
    assert_ne!(with, cache_of(&root, &["props", "crate.fbx"]));
}

/// A path component: letters of either case, `\`, `.` and `_`, never `.` or `..`.
fn component() -> impl Strategy<Value = String> {
    "[aAbB\\\\._]{1,3}".prop_filter("a name, not . or ..", |c| c != "." && c != "..")
}

/// A project-relative source path, `assets/` first or not.
fn source() -> impl Strategy<Value = Vec<String>> {
    (any::<bool>(), prop::collection::vec(component(), 1..4)).prop_map(|(assets, mut parts)| {
        if assets {
            parts.insert(0, "assets".into());
        }
        parts
    })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    /// Distinct sources, distinct caches: the cache path keeps the whole source
    /// path, a `\` included (part of a file name on Linux and macOS).
    #[test]
    fn distinct_sources_get_distinct_caches(a in source(), b in source()) {
        let root = project();
        prop_assert_eq!(cache_of(&root, &a) == cache_of(&root, &b), a == b);
    }
}

#[test]
fn project_relative_keeps_a_backslash_and_drops_a_leading_dot() {
    let root = project();
    let rel = |p: &str| project_relative(&root, Path::new(p));
    assert_eq!(rel(r"a\b.fbx").as_deref(), Some(r"a\b.fbx"));
    assert_eq!(rel("./a/b.fbx").as_deref(), Some("a/b.fbx"));
    assert_eq!(rel("../a.fbx"), None);
    assert_ne!(cache_of(&root, &["A.fbx"]), cache_of(&root, &["a.fbx"]));
}
