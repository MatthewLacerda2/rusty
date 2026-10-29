//! Linked instances: `SavePrefab` → `Instantiate`, and overrides and their revert.

use super::{fixture, is_linked, pos_of, run};
use glam::Vec3;

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

        // A pristine linked instance has no divergence.
        let overrides: Vec<String> = lua
            .load(format!("return Scene.ListPrefabOverrides({new_id})"))
            .eval()
            .unwrap();
        assert!(overrides.is_empty());
    });
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_linked_instance_edit_is_an_override_and_reverts() {
    let (scene, root, path) = fixture("overrides");
    run(&scene, |lua| {
        let new_id: u32 = lua
            .load(format!(
                "Scene.SavePrefab({root}, '{path}')\n\
                 return Scene.Instantiate('{path}')"
            ))
            .eval()
            .unwrap();

        // Diverge from the source: the divergence is listed with nothing recorded.
        lua.load(format!("Transform.SetLocalScale({new_id}, 9, 9, 9)"))
            .exec()
            .unwrap();
        let overrides: Vec<String> = lua
            .load(format!("return Scene.ListPrefabOverrides({new_id})"))
            .eval()
            .unwrap();
        assert_eq!(overrides.len(), 3, "{overrides:?}");

        // Revert restores the source values.
        lua.load(format!("Scene.RevertPrefabOverrides({new_id})"))
            .exec()
            .unwrap();
        let scale = scene.borrow().world.transform(new_id).unwrap().scale;
        assert_eq!(scale, Vec3::ONE);
    });
    let _ = std::fs::remove_file(&path);
}
