//! `Scene.RevertPrefabField` (#998): drop one override so the leaf re-tracks the
//! source. `true` when an override went, `false` when there was none to drop.

use super::{errors, fixture, run};
use glam::Vec3;

#[test]
fn revert_prefab_field_returns_whether_it_reverted() {
    let (scene, root, path) = fixture("revert_field");
    run(&scene, |lua| {
        let id: u32 = lua
            .load(format!(
                "Scene.SavePrefab({root}, '{path}')\n\
                 return Scene.Instantiate('{path}')"
            ))
            .eval()
            .unwrap();
        lua.load(format!("Transform.SetLocalScale({id}, 9, 1, 1)"))
            .exec()
            .unwrap();

        let revert = format!("return Scene.RevertPrefabField({id}, '/transform/scale/0')");
        let first: bool = lua.load(&revert).eval().unwrap();
        assert!(first, "the X override was dropped");
        let scale = scene.borrow().world.transform(id).unwrap().scale;
        assert_eq!(scale, Vec3::ONE);
        let second: bool = lua.load(&revert).eval().unwrap();
        assert!(!second, "nothing left to revert");
    });
    let _ = std::fs::remove_file(&path);
}

#[test]
fn revert_prefab_field_on_an_unlinked_entity_errors() {
    let (scene, root, _) = fixture("revert_field_unlinked");
    run(&scene, |lua| {
        assert!(errors(
            lua,
            &format!("Scene.RevertPrefabField({root}, '/name')")
        ));
    });
}
