use super::{check, RULE};

fn flagged(path: &str, src: &str) -> usize {
    check(path, src).len()
}

#[test]
fn flags_a_header_naming_another_file() {
    let src = "//! src/scene/prefab_link.rs — the link.\n";
    let found = check("src/scene/prefab/link.rs", src);
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
    assert_eq!(
        flagged("src/a.rs", "//! tools/lint — a tool name, not a path.\n"),
        0
    );
    let sh = "// engine/shaders/a.wgsl — x\nfn f() {}\n";
    assert_eq!(flagged("engine/shaders/a.wgsl", sh), 0);
    // A plain `//` comment further down a shader is no header.
    let low = "fn f() {}\n// src/other.rs is elsewhere\n";
    assert_eq!(flagged("engine/shaders/a.wgsl", low), 0);
}
