//! Linked instances: `SavePrefab` → `Instantiate`, and override record/revert.

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
