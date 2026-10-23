//! The copy and the output checks, on a scratch project (no working-directory
//! move: the conversion half runs through the `export` binary, `tests/export/`).

mod check;

use super::copy_runtime_files;
use std::path::{Path, PathBuf};

/// The import cache of `assets/models/<name>`.
const CACHE: &str = "assets/generated/imported/assets/models";

/// A project holding one file of each kind the copy decides about.
fn project(root: &Path) {
    let cached = |file: &str| format!("{CACHE}/{file}");
    for (rel, text) in [
        ("project.rusty", "{}".to_string()),
        (".seeded", "{}".into()),
        ("cache/thumb.png", "machine-local".into()),
        ("saved/slot1.json", "this machine's save".into()),
        ("assets/scenes/main.scene", "scene".into()),
        ("assets/scripts/b.lua", "b".into()),
        ("assets/scripts/a.lua", "a".into()),
        ("assets/models/soldier.fbx", "source".into()),
        ("assets/models/crate.blend", "source".into()),
        ("assets/models/crate.blend1", "Blender's backup".into()),
        ("assets/models/crate.blend12", "an older backup".into()),
        ("assets/models/crate.blend@", "Blender's temp file".into()),
        ("assets/models/barrel.glb", "glb".into()),
        ("assets/models/rock.obj", "obj".into()),
        ("assets/generated/voice.wav", "provider output".into()),
        (&cached("soldier.fbx.glb"), "cache".into()),
        (&cached("soldier.fbx.glb.source"), "stamp".into()),
        (
            &cached("soldier.fbx.glb.0123456789abcdef.textures/skin.png"),
            "texture".into(),
        ),
        (&cached("gone.fbx.glb"), "an orphan cache".into()),
    ] {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
}

/// The sources [`project`] holds, as `export` finds them.
fn sources() -> Vec<PathBuf> {
    ["assets/models/crate.blend", "assets/models/soldier.fbx"]
        .map(PathBuf::from)
        .to_vec()
}

#[test]
fn the_runtime_files_are_copied_in_order_and_the_sources_left_behind() {
    let dir = crate::test_temp::dir();
    let (root, out) = (dir.join("game"), dir.join("out"));
    project(&root);
    let files = copy_runtime_files(&root, &sources(), &out).unwrap();
    let cached = |file: &str| format!("{CACHE}/{file}");
    assert_eq!(
        files,
        [
            "project.rusty",
            ".seeded",
            "assets/generated/voice.wav",
            "assets/models/barrel.glb",
            "assets/models/rock.obj",
            "assets/scenes/main.scene",
            "assets/scripts/a.lua",
            "assets/scripts/b.lua",
            &cached("soldier.fbx.glb"),
            &cached("soldier.fbx.glb.0123456789abcdef.textures/skin.png"),
            &cached("soldier.fbx.glb.source"),
        ]
    );
    for file in &files {
        let (from, to) = (root.join(file), out.join(file));
        assert_eq!(std::fs::read(from).unwrap(), std::fs::read(to).unwrap());
    }
    for gone in ["soldier.fbx", "crate.blend", "crate.blend1", "crate.blend@"] {
        let gone = format!("assets/models/{gone}");
        assert!(!out.join(&gone).exists(), "{gone} is not exported");
    }
    let orphan = cached("gone.fbx.glb");
    for gone in ["cache", "saved", &orphan] {
        assert!(!out.join(gone).exists(), "{gone} is not exported");
    }
}

#[cfg(unix)]
#[test]
fn a_symlinked_folder_fails_the_export_naming_it() {
    let dir = crate::test_temp::dir();
    let (root, out) = (dir.join("game"), dir.join("out"));
    project(&root);
    std::fs::create_dir_all(dir.join("shared")).unwrap();
    std::os::unix::fs::symlink(dir.join("shared"), root.join("assets/shared")).unwrap();
    let err = copy_runtime_files(&root, &sources(), &out).unwrap_err();
    assert!(err.contains("assets/shared: a symlinked folder"), "{err}");
}
