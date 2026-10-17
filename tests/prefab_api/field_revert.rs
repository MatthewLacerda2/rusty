//! `Scene.RevertPrefabField` (#998): one override reverts, the rest stay.

use super::{errors, fixture, pos_of, run};
use glam::Vec3;

#[test]
fn revert_one_field_keeps_the_other_overrides() {
    let (scene, root, path) = fixture("field_revert");
    run(&scene, |lua| {
        let id: u32 = lua
            .load(format!(
                "Scene.SavePrefab({root}, '{path}')\n\
                 return Scene.Instantiate('{path}')"
            ))
            .eval()
            .unwrap();
        scene.borrow_mut().world.transform_mut(id).unwrap().position = Vec3::splat(9.0);
        let revert = |pointer: &str| -> bool {
            let call = format!("return Scene.RevertPrefabField({id}, '{pointer}')");
            lua.load(call).eval().unwrap()
        };
        lua.load(format!("Scene.RecordPrefabOverrides({id})"))
            .exec()
            .unwrap();

        assert!(revert("/transform/position/0"), "X was overridden");
        assert_eq!(pos_of(&scene, id), Vec3::new(1.0, 9.0, 9.0));
        assert!(!revert("/transform/position/0"), "nothing left to revert");
        assert!(errors(
            lua,
            &format!("Scene.RevertPrefabField({root}, '/name')")
        ));
    });
    let _ = std::fs::remove_file(&path);
}
