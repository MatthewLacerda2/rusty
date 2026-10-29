//! #1527: a Lua edit to a linked instance is an override the moment it is made, and
//! leaving Play or a save/load round trip never takes it back.

use super::{fixture, pos_of, run};
use glam::Vec3;
use rusty::scene::snapshot::SceneSnapshot;
use rusty::scene::{apply_scene_data, to_scene_data, Scene, SceneData};

/// `scene` saved to JSON and loaded back into a fresh one, as a scene file is.
fn reload(scene: &Scene) -> Scene {
    let json = serde_json::to_string(&to_scene_data(scene)).unwrap();
    let mut back = Scene::new();
    apply_scene_data(&mut back, serde_json::from_str::<SceneData>(&json).unwrap());
    back
}

#[test]
fn a_moved_instance_root_stays_put_after_play() {
    let (scene, root, path) = fixture("instant_root");
    run(&scene, |lua| {
        let p: u32 = lua
            .load(format!(
                "Scene.SavePrefab({root}, '{path}')\n\
                 local p = Scene.Instantiate('{path}')\n\
                 Transform.SetPosition(p, 5, 3.5, -20)\n\
                 return p"
            ))
            .eval()
            .unwrap();
        let listed: Vec<String> = lua
            .load(format!("return Scene.ListPrefabOverrides({p})"))
            .eval()
            .unwrap();
        assert!(listed.is_empty(), "the root's placement is no override");

        // Play, move it in Play, Stop.
        let snapshot = SceneSnapshot::capture(&scene.borrow());
        scene
            .borrow_mut()
            .world
            .transform_mut(p)
            .unwrap()
            .position
            .y = 3.38;
        snapshot.restore(&mut scene.borrow_mut());
        assert_eq!(pos_of(&scene, p), Vec3::new(5.0, 3.5, -20.0));
        let back = reload(&scene.borrow());
        let pos = back.world.transform(p).unwrap().position;
        assert_eq!(pos, Vec3::new(5.0, 3.5, -20.0), "and after a save/load");
    });
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_lua_edit_to_a_child_survives_play_and_a_reload() {
    let (scene, root, path) = fixture("instant_child");
    let child = scene.borrow_mut().add_entity("Child".to_string());
    scene.borrow_mut().set_parent(child, Some(root)).unwrap();
    run(&scene, |lua| {
        let c: u32 = lua
            .load(format!(
                "Scene.SavePrefab({root}, '{path}')\n\
                 local p = Scene.Instantiate('{path}')\n\
                 local c = Scene.GetChildren(p)[1]\n\
                 Transform.SetLocalPosition(c, 4, 0, 0)\n\
                 return c"
            ))
            .eval()
            .unwrap();
        let p = scene.borrow().world.parent_id(c).unwrap();
        let listed: Vec<String> = lua
            .load(format!("return Scene.ListPrefabOverrides({p})"))
            .eval()
            .unwrap();
        assert_eq!(listed, ["1:/transform/position/0"]);

        let snapshot = SceneSnapshot::capture(&scene.borrow());
        scene
            .borrow_mut()
            .world
            .transform_mut(c)
            .unwrap()
            .position
            .x = 40.0;
        snapshot.restore(&mut scene.borrow_mut());
        let back = reload(&scene.borrow());
        assert_eq!(back.world.transform(c).unwrap().position.x, 4.0);
    });
    let _ = std::fs::remove_file(&path);
}
