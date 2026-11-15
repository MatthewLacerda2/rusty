//! The engine content digest and the stamp a run carries.

use super::*;
use std::path::{Path, PathBuf};

/// A fresh folder under the test's temp dir holding `files` (`path`, `bytes`).
fn tree(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = rusty_test_support::temp::dir().join(name);
    let _ = std::fs::remove_dir_all(&dir);
    for (rel, bytes) in files {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    dir
}

const BASE: [(&str, &str); 3] = [
    ("shaders/common.wgsl", "fn a() {}"),
    ("shaders/post/ssr.wgsl", "fn b() {}"),
    ("fonts/ui.txt", "font"),
];

#[test]
fn the_digest_is_of_content_not_of_where_the_folder_is() {
    let one = digest(&tree("digest_one", &BASE)).unwrap();
    let two = digest(&tree("digest_two", &BASE)).unwrap();
    assert_eq!(one, two);
    assert_eq!(one.len(), 16, "{one}");
    assert!(one.chars().all(|c| c.is_ascii_hexdigit()), "{one}");
}

#[test]
fn a_changed_byte_a_renamed_file_or_a_new_file_changes_the_digest() {
    let base = digest(&tree("digest_base", &BASE)).unwrap();
    let mut edited = BASE;
    edited[1].1 = "fn c() {}";
    let mut renamed = BASE;
    renamed[1].0 = "shaders/post/ss.wgsl";
    let added = [BASE[0], BASE[1], BASE[2], ("models/cube.gltf", "")];
    // Bytes moved from a file into its name: the same bytes in a row, another tree.
    let moved = [("shaders/common.wgsl", "fn a() {"), ("shaders/x", "}")];
    let unmoved = [("shaders/common.wgsl", "fn a() {}"), ("shaders/x", "")];
    for (name, files) in [
        ("digest_edited", &edited[..]),
        ("digest_renamed", &renamed),
        ("digest_added", &added),
    ] {
        assert_ne!(digest(&tree(name, files)), Some(base.clone()), "{name}");
    }
    assert_ne!(
        digest(&tree("digest_moved", &moved)),
        digest(&tree("digest_unmoved", &unmoved))
    );
}

#[test]
fn a_hidden_file_is_not_content() {
    let base = digest(&tree("digest_plain", &BASE)).unwrap();
    let hidden = [
        BASE[0],
        BASE[1],
        BASE[2],
        (".DS_Store", "x"),
        ("shaders/.swp", "y"),
    ];
    assert_eq!(digest(&tree("digest_hidden", &hidden)), Some(base));
}

#[test]
fn a_missing_or_empty_folder_has_no_digest() {
    assert_eq!(digest(Path::new("/no/such/engine/folder")), None);
    assert_eq!(digest(&tree("digest_empty", &[])), None);
}

#[test]
fn the_folder_this_build_reads_is_hashed_like_any_other() {
    let engine = crate::project::engine_dir();
    assert_eq!(on_disk(), digest(engine).as_deref());
    assert!(on_disk().is_some(), "{} has a digest", engine.display());
}

#[test]
fn a_stamp_matches_only_when_both_digests_are_known_and_equal() {
    let matches = |b, f| EngineStamp::of(b, f).content_matches;
    assert_eq!(matches(Some("a"), Some("a")), Some(true));
    assert_eq!(matches(Some("a"), Some("b")), Some(false));
    assert_eq!(matches(None, Some("b")), None);
    assert_eq!(matches(Some("a"), None), None);
    assert_eq!(EngineStamp::of(None, None).commit, engine_commit());
}

#[test]
fn only_a_mismatch_warns_and_the_warning_names_both_digests() {
    assert_eq!(EngineStamp::of(Some("a"), Some("a")).warning(), None);
    assert_eq!(EngineStamp::of(None, Some("b")).warning(), None);
    let warning = EngineStamp::of(Some("0123abcd"), Some("feed9876"))
        .warning()
        .unwrap();
    for part in ["0123abcd", "feed9876", "Rebuild"] {
        assert!(warning.contains(part), "{part} in {warning}");
    }
    let engine = crate::project::engine_dir().display().to_string();
    assert!(warning.contains(&engine), "{warning}");
}

#[test]
fn the_stamp_serialises_with_every_field_named() {
    let json = serde_json::to_value(EngineStamp::of(Some("a"), Some("b"))).unwrap();
    for key in [
        "commit",
        "content_built",
        "content_found",
        "content_matches",
    ] {
        assert!(json.get(key).is_some(), "{key} in {json}");
    }
}

#[test]
fn registering_another_build_warns_once_and_keeps_the_first() {
    let other = "0000000000000000";
    let logged = rusty_test_support::logs::during(|| {
        register_built(other);
        register_built(on_disk().unwrap());
    });
    assert_eq!(stamp().content_built.as_deref(), Some(other));
    assert_eq!(stamp().content_matches, Some(false));
    let warnings: Vec<_> = logged.iter().filter(|l| l.contains(other)).collect();
    assert_eq!(warnings.len(), 1, "{logged:?}");
}
