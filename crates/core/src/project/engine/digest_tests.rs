use super::digest;
use std::path::{Path, PathBuf};

/// A fresh folder under the test's temp dir holding `files` (`path`, `bytes`).
fn tree(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = rusty_test_support::temp::dir().join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
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
    for (name, files) in [
        ("digest_edited", &edited[..]),
        ("digest_renamed", &renamed),
        ("digest_added", &added),
    ] {
        assert_ne!(digest(&tree(name, files)), Ok(base.clone()), "{name}");
    }
}

#[test]
fn bytes_moved_across_a_file_boundary_change_the_digest() {
    let one = digest(&tree("digest_moved_one", &[("a", "b\0X")]));
    let two = digest(&tree("digest_moved_two", &[("a", ""), ("b", "X")]));
    assert_ne!(one.unwrap(), two.unwrap());
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
    assert_eq!(digest(&tree("digest_hidden", &hidden)), Ok(base));
}

#[test]
fn a_missing_or_empty_folder_has_no_digest_and_says_why() {
    let missing = digest(Path::new("/no/such/engine/folder")).unwrap_err();
    assert!(missing.contains("/no/such/engine/folder"), "{missing}");
    let empty = tree("digest_empty", &[]);
    assert!(empty.is_dir());
    let why = digest(&empty).unwrap_err();
    assert!(why.contains("holds no file"), "{why}");
}

#[test]
fn a_link_is_hashed_as_what_it_names_and_a_dangling_one_says_why() {
    let base = digest(&tree("digest_copied", &BASE)).unwrap();
    let real = tree("digest_link_target", &BASE[..2]);
    let linked = tree("digest_linked", &[BASE[2]]);
    std::os::unix::fs::symlink(real.join("shaders"), linked.join("shaders")).unwrap();
    assert_eq!(digest(&linked), Ok(base));
    let dangling = tree("digest_dangling", &BASE);
    std::os::unix::fs::symlink("/no/such/file", dangling.join("gone.wgsl")).unwrap();
    let why = digest(&dangling).unwrap_err();
    assert!(why.contains("gone.wgsl"), "{why}");
}
