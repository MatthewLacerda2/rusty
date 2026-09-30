//! What the prefab verbs refuse: linked-only verbs on a plain copy, and bad arguments.

use super::{errors, fixture, is_linked, run};

#[test]
fn unpacked_copy_carries_no_link() {
    let (scene, root, path) = fixture("unpacked");
    run(&scene, |lua| {
        let new_id: u32 = lua
            .load(format!(
                "Scene.SavePrefab({root}, '{path}')\n\
                 return Scene.InstantiateUnpacked('{path}')"
            ))
            .eval()
            .unwrap();
        assert!(!is_linked(&scene, new_id), "an unpacked copy has no link");
        // The linked-instance verbs must reject a plain copy, not silently no-op.
        assert!(errors(
            lua,
            &format!("Scene.RecordPrefabOverrides({new_id})")
        ));
    });
    let _ = std::fs::remove_file(&path);
}

#[test]
fn error_paths_reject_cleanly() {
    let (scene, root, path) = fixture("errors");
    run(&scene, |lua| {
        let save_bad_root = format!("Scene.SavePrefab(9999, '{path}')");
        assert!(errors(lua, &save_bad_root), "saving a missing root errors");
        let missing = "Scene.Instantiate('/nope/missing.prefab')";
        assert!(errors(lua, missing), "instantiating a missing file errors");

        lua.load(format!("Scene.SavePrefab({root}, '{path}')"))
            .exec()
            .unwrap();
        let bad_parent = format!("Scene.Instantiate('{path}', 'abc')");
        assert!(errors(lua, &bad_parent), "a non-numeric parentId errors");
    });
    let _ = std::fs::remove_file(&path);
}
