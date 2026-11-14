use super::*;

#[test]
fn a_crate_file_reads_at_its_module_path() {
    assert_eq!(module_path("crates/core/src/lib.rs"), "src/core/mod.rs");
    assert_eq!(
        module_path("crates/core/src/fs/mod.rs"),
        "src/core/fs/mod.rs"
    );
    assert_eq!(module_path("crates/time/src/a/b.rs"), "src/time/a/b.rs");
    assert_eq!(module_path("crates/time/src"), "src/time");
}

#[test]
fn other_paths_keep_their_own() {
    for p in [
        "src/bin/play.rs",
        "crates/core/build.rs",
        "crates/core/Cargo.toml",
        "crates/test-support/src/temp.rs",
        "tests/main.rs",
        "cratesx/a/src/lib.rs",
    ] {
        assert_eq!(module_path(p), p);
    }
}

#[test]
fn a_member_that_is_no_module_has_none() {
    assert_eq!(crate_module("crates/core/src/lib.rs"), Some("core"));
    assert_eq!(crate_module("crates/test-support/src/lib.rs"), None);
    assert_eq!(crate_module("src/bin/play.rs"), None);
}

#[test]
fn a_module_is_found_where_it_lives() {
    let root = std::env::temp_dir().join(format!("lint-modules-{}", std::process::id()));
    fs::create_dir_all(root.join("crates/core/src")).unwrap();
    fs::create_dir_all(root.join("crates/test-support/src")).unwrap();
    fs::create_dir_all(root.join("src/bin")).unwrap();
    assert_eq!(dir(&root, "core"), root.join("crates/core/src"));
    assert_eq!(all(&root), ["core"]);
    assert_eq!(
        real(&root, "src/core/mod.rs"),
        root.join("crates/core/src/lib.rs")
    );
    assert_eq!(
        real(&root, "src/core/fs/a.rs"),
        root.join("crates/core/src/fs/a.rs")
    );
    assert_eq!(real(&root, "src/core"), root.join("crates/core/src"));
    assert_eq!(real(&root, "src/bin/play.rs"), root.join("src/bin/play.rs"));
    fs::remove_dir_all(&root).ok();
}
