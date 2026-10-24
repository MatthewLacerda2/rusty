//! What `export` refuses before it opens the project or writes anything.

use super::super::{check, check_out};
use super::project;

#[test]
fn an_output_that_holds_anything_is_refused() {
    let dir = crate::test_temp::dir();
    let (root, out) = (dir.join("game"), dir.join("out"));
    project(&root);
    assert!(check_out(&root, &out).is_ok(), "an absent folder");
    std::fs::create_dir_all(&out).unwrap();
    assert!(check_out(&root, &out).is_ok(), "an empty folder");
    std::fs::write(out.join("old.txt"), "").unwrap();
    let err = check_out(&root, &out).unwrap_err();
    assert!(err.contains("not empty"), "{err}");
}

#[test]
fn an_output_that_is_a_file_is_refused() {
    let dir = crate::test_temp::dir();
    let (root, out) = (dir.join("game"), dir.join("out.zip"));
    project(&root);
    std::fs::write(&out, "").unwrap();
    let err = check_out(&root, &out).unwrap_err();
    assert!(err.contains("a file"), "{err}");
}

#[test]
fn an_output_overlapping_the_project_is_refused() {
    let dir = crate::test_temp::dir();
    let root = dir.join("game");
    project(&root);
    for out in [root.join("build").join("x"), root.clone(), dir.clone()] {
        let err = check_out(&root, &out).unwrap_err();
        assert!(err.contains("outside the project"), "{err}");
    }
}

#[cfg(unix)]
#[test]
fn an_output_reaching_the_project_through_a_symlink_is_refused() {
    let dir = crate::test_temp::dir();
    let root = dir.join("game");
    project(&root);
    std::os::unix::fs::symlink(&root, dir.join("link")).unwrap();
    let err = check_out(&root, &dir.join(crate::test_path::native("link/build/new"))).unwrap_err();
    assert!(err.contains("outside the project"), "{err}");
}

#[test]
fn a_folder_that_is_not_a_project_is_refused() {
    let dir = crate::test_temp::dir();
    let (stray, out) = (dir.join("typo"), dir.join("out"));
    std::fs::create_dir_all(&stray).unwrap();
    let err = check(&stray, &out).unwrap_err();
    assert!(err.contains("not a rusty project"), "{err}");
    let err = check(&dir.join("missing"), &out).unwrap_err();
    assert!(err.contains("not a rusty project"), "{err}");
}
