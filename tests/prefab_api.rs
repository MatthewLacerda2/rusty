//! Integration coverage for the `Scene` namespace's prefab verbs (issue #350):
//! `SavePrefab` → `Instantiate` round-trips an entity through a `.prefab` file,
//! linked instances record/revert overrides, unpacked copies carry no link, and
//! the error paths (bad root, missing file, bad parent arg) reject cleanly.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;
use rusty::scene::Scene;

/// A scene whose root entity sits at (1, 2, 3), plus the tmp `.prefab` path this
/// test round-trips through (unique per test: they run in parallel). The path is
/// interpolated into single-quoted Lua strings, where a Windows `\` would be
/// taken as an escape sequence — so it is normalized to `/`, which `std::fs`
/// accepts on every platform.
fn fixture(tag: &str) -> (RefCell<Scene>, u32, String) {
    let mut scene = Scene::new();
    let root = scene.add_entity("Root".to_string());
    scene.world.transform_mut(root).unwrap().position = Vec3::new(1.0, 2.0, 3.0);
    let dir = std::env::temp_dir();
    let file = format!("rusty_prefab_api_{tag}_{}.prefab", std::process::id());
    let path = dir.join(file).to_string_lossy().replace('\\', "/");
    (RefCell::new(scene), root, path)
}

fn run(scene: &RefCell<Scene>, body: impl FnOnce(&Lua)) {
    let lua = Lua::new();
    let scene_path: RefCell<Option<String>> = RefCell::new(None);
    let is_playing = RefCell::new(false);
    lua.scope(|scope| {
        rusty::api::scene::register(&lua, scope, scene, &scene_path, &is_playing).unwrap();
        body(&lua);
        Ok(())
    })
    .unwrap();
}

fn pos_of(scene: &RefCell<Scene>, id: u32) -> Vec3 {
    scene.borrow().world.transform(id).unwrap().position
}

fn is_linked(scene: &RefCell<Scene>, id: u32) -> bool {
    scene.borrow().world.has_prefab_link(id)
}

fn errors(lua: &Lua, code: &str) -> bool {
    lua.load(code).exec().is_err()
}

#[test]
fn save_then_instantiate_linked_round_trips() {
    let (scene, root, path) = fixture("linked");
    run(&scene, |lua| {
        let saved: String = lua
            .load(format!("return Scene.SavePrefab({root}, '{path}')"))
            .eval()
            .unwrap();
        assert_eq!(saved, path);

        let new_id: u32 = lua
            .load(format!("return Scene.Instantiate('{path}')"))
            .eval()
            .unwrap();
        assert_ne!(new_id, root, "a fresh instance, not the original");
        assert_eq!(pos_of(&scene, new_id), Vec3::new(1.0, 2.0, 3.0));
        assert!(is_linked(&scene, new_id), "Instantiate links by default");

        // A pristine linked instance has no recorded divergence.
        let overrides: Vec<String> = lua
            .load(format!("return Scene.ListPrefabOverrides({new_id})"))
            .eval()
            .unwrap();
        assert!(overrides.is_empty());
    });
    let _ = std::fs::remove_file(&path);
}

#[test]
fn linked_instance_records_and_reverts_overrides() {
    let (scene, root, path) = fixture("overrides");
    run(&scene, |lua| {
        let new_id: u32 = lua
            .load(format!(
                "Scene.SavePrefab({root}, '{path}')\n\
                 return Scene.Instantiate('{path}')"
            ))
            .eval()
            .unwrap();

        // Diverge from the source, record, and the divergence is listed.
        {
            let mut s = scene.borrow_mut();
            s.world.transform_mut(new_id).unwrap().position = Vec3::splat(9.0);
        }
        let recorded: usize = lua
            .load(format!("return Scene.RecordPrefabOverrides({new_id})"))
            .eval()
            .unwrap();
        assert!(recorded > 0, "the moved transform is a recorded override");
        let overrides: Vec<String> = lua
            .load(format!("return Scene.ListPrefabOverrides({new_id})"))
            .eval()
            .unwrap();
        assert!(!overrides.is_empty());

        // Revert restores the source values.
        lua.load(format!("Scene.RevertPrefabOverrides({new_id})"))
            .exec()
            .unwrap();
        assert_eq!(pos_of(&scene, new_id), Vec3::new(1.0, 2.0, 3.0));
    });
    let _ = std::fs::remove_file(&path);
}

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
