use super::{check, dirs_of, RULE};
use std::collections::BTreeSet;

/// The directories of a small tree: `src/a/b/`, the tool `tools/lint`, and `core`
/// moved to its crate.
fn tree() -> BTreeSet<String> {
    let files = [
        "src/a/b/mod.rs",
        "tools/lint/src/main.rs",
        "crates/core/src/lib.rs",
    ];
    dirs_of(&files.map(String::from))
}

fn flagged(path: &str, src: &str) -> usize {
    check(path, src, &tree()).len()
}

#[test]
fn flags_a_header_naming_another_file() {
    let src = "//! src/scene/prefab_link.rs — the link.\n";
    let found = check("src/scene/prefab/link.rs", src, &tree());
    assert_eq!(found.len(), 1);
    assert_eq!((found[0].line, found[0].rule), (1, RULE));
    assert!(found[0].message.contains("src/scene/prefab/link.rs"));
    // A mod.rs naming a different directory, and a backticked path.
    assert_eq!(flagged("src/a/b/mod.rs", "//! src/a/old/ — x.\n"), 1);
    assert_eq!(flagged("src/a/b.rs", "//! `src/a/c.rs`: x.\n"), 1);
    // The header after an inner attribute, and a stale shader header.
    assert_eq!(flagged("src/a.rs", "#![deny(x)]\n//! src/b.rs — x.\n"), 1);
    let sh = "// engine/shaders/old.wgsl — x\n";
    assert_eq!(flagged("engine/shaders/new/a.wgsl", sh), 1);
}

#[test]
fn passes_its_own_path_a_directory_and_no_path() {
    let own = "//! src/scene/prefab_link.rs — x.\n";
    assert_eq!(flagged("src/scene/prefab_link.rs", own), 0);
    for head in ["src/a/b/", "src/a/b", "src/a/b/mod.rs", "`src/a/b/`"] {
        let src = format!("//! {head} — x.\n");
        assert_eq!(flagged("src/a/b/mod.rs", &src), 0, "{head}");
    }
    assert_eq!(
        flagged("src/a.rs", "//! Unit tests for `GameWorld` (game.rs).\n"),
        0
    );
    assert_eq!(flagged("src/a.rs", "fn f() {}\n"), 0);
    assert_eq!(flagged("src/a.rs", "//! The scene/ folder.\n"), 0);
    // A bare word naming no directory of the tree is prose.
    assert_eq!(flagged("src/a.rs", "//! tools/fmt — no such folder.\n"), 0);
    let sh = "// engine/shaders/a.wgsl — x\nfn f() {}\n";
    assert_eq!(flagged("engine/shaders/a.wgsl", sh), 0);
    // A plain `//` comment further down a shader is no header.
    let low = "fn f() {}\n// src/other.rs is elsewhere\n";
    assert_eq!(flagged("engine/shaders/a.wgsl", low), 0);
}

#[test]
fn a_bare_directory_header_is_a_path_when_the_directory_exists() {
    // Its real path and its module path both count, so a moved `lib.rs` still
    // naming `src/<m>` is stale, and so is a bare directory of another file.
    assert_eq!(flagged("crates/core/src/lib.rs", "//! src/core — x.\n"), 1);
    assert_eq!(
        flagged("crates/core/src/x.rs", "//! crates/core/src — x.\n"),
        1
    );
    assert_eq!(flagged("src/a.rs", "//! tools/lint — x.\n"), 1);
    assert_eq!(flagged("src/a/c/mod.rs", "//! `src/a/b`: x.\n"), 1);
    // Its own directory passes, as with a trailing `/`.
    assert_eq!(flagged("src/a/b/mod.rs", "//! src/a/b — x.\n"), 0);
    assert!(tree().contains("src/core") && tree().contains("crates/core/src"));
}
