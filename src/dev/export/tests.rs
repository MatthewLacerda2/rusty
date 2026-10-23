//! The copy and the output checks, on a scratch project (no working-directory
//! move: the conversion half runs through the `export` binary, `tests/export/`).

use super::{check_out, copy_runtime_files};
use std::path::Path;

/// A project holding one file of each kind the copy decides about.
fn project(root: &Path) {
    for (rel, text) in [
        ("project.rusty", "{}"),
        (".seeded", "{}"),
        ("cache/thumb.png", "machine-local"),
        ("saved/slot1.json", "this machine's save"),
        ("assets/scenes/main.scene", "scene"),
        ("assets/scripts/b.lua", "b"),
        ("assets/scripts/a.lua", "a"),
        ("assets/models/soldier.fbx", "source"),
        ("assets/models/crate.blend", "source"),
        ("assets/models/barrel.glb", "glb"),
        ("assets/models/rock.obj", "obj"),
        (
            "assets/generated/imported/assets/models/soldier.fbx.glb",
            "cache",
        ),
    ] {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
}

#[test]
fn the_runtime_files_are_copied_in_order_and_the_sources_left_behind() {
    let dir = crate::test_temp::dir();
    let (root, out) = (dir.join("game"), dir.join("out"));
    project(&root);
    let files = copy_runtime_files(&root, &out).unwrap();
    assert_eq!(
        files,
        [
            "project.rusty",
            ".seeded",
            "assets/generated/imported/assets/models/soldier.fbx.glb",
            "assets/models/barrel.glb",
            "assets/models/rock.obj",
            "assets/scenes/main.scene",
            "assets/scripts/a.lua",
            "assets/scripts/b.lua",
        ]
    );
    for file in &files {
        let (from, to) = (root.join(file), out.join(file));
        assert_eq!(std::fs::read(from).unwrap(), std::fs::read(to).unwrap());
    }
    for gone in [
        "assets/models/soldier.fbx",
        "assets/models/crate.blend",
        "cache",
        "saved",
    ] {
        assert!(!out.join(gone).exists(), "{gone} is not exported");
    }
}

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
fn an_output_overlapping_the_project_is_refused() {
    let dir = crate::test_temp::dir();
    let root = dir.join("game");
    project(&root);
    for out in [root.join("build"), root.clone(), dir.clone()] {
        let err = check_out(&root, &out).unwrap_err();
        assert!(err.contains("outside the project"), "{err}");
    }
}
