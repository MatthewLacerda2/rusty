//! The atomic write (#1129): no torn file, no litter, no collision.

use super::{is_temp_name, is_unique_sibling, unique_sibling, write_atomic, TEMP_SUFFIX};
use crate::core::paths::native;
use std::path::Path;

/// Every file name in `dir`, sorted.
fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn a_write_replaces_the_file_and_leaves_nothing_beside_it() {
    let dir = crate::test_temp::dir();
    let path = dir.join("a.txt");
    write_atomic(&path, b"one").unwrap();
    write_atomic(&path, b"two").unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), b"two");
    assert_eq!(names(&dir), ["a.txt"]);
}

#[test]
fn a_failed_write_leaves_no_temp_file() {
    let dir = crate::test_temp::dir();
    // A non-empty directory in the way: the temp file is written, the rename fails.
    let path = dir.join("taken");
    std::fs::create_dir_all(path.join("inside")).unwrap();
    assert!(write_atomic(&path, b"lost").is_err());
    assert_eq!(names(&dir), ["taken"]);
}

#[test]
fn two_writers_in_one_process_never_collide() {
    let dir = crate::test_temp::dir();
    let path = dir.join("shared.txt");
    let writers: Vec<_> = [b'a', b'b']
        .into_iter()
        .map(|byte| {
            let path = path.clone();
            std::thread::spawn(move || {
                for _ in 0..200 {
                    write_atomic(&path, &[byte; 4096]).expect("no collision");
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().unwrap();
    }
    let bytes = std::fs::read(&path).unwrap();
    assert!(
        bytes == [b'a'; 4096] || bytes == [b'b'; 4096],
        "a torn file"
    );
    assert_eq!(names(&dir), ["shared.txt"]);
}

#[test]
fn a_unique_sibling_is_recognised_and_other_names_are_not() {
    let made = unique_sibling(&native("dir/a.glb"), "staging");
    let name = made.file_name().unwrap().to_string_lossy().into_owned();
    assert_eq!(made.parent(), Some(Path::new("dir")));
    assert!(is_unique_sibling(&name, "a.glb", "staging"));
    assert!(!is_unique_sibling(&name, "a.glb", "tmp"));
    assert!(!is_unique_sibling(&name, "b.glb", "staging"));
    assert!(!is_unique_sibling(
        "a.glb.x.fbx.1.2.staging",
        "a.glb",
        "staging"
    ));
    assert!(!is_unique_sibling("a.glb.1.staging", "a.glb", "staging"));
}

#[test]
fn only_a_write_in_flight_is_a_temp_name() {
    let made = unique_sibling(&native("dir/a.png"), TEMP_SUFFIX);
    assert!(is_temp_name(&made.file_name().unwrap().to_string_lossy()));
    for name in [
        "a.png",
        "a.tmp",
        "a.1.tmp",
        "a.x.1.tmp",
        ".1.2.tmp",
        "a.1.2.staging",
    ] {
        assert!(!is_temp_name(name), "{name}");
    }
}
