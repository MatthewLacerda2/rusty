//! Each setter writes the scene the editor reads, and the scene round-trips it.

use std::cell::RefCell;

use rusty::components::CameraComponent;
use rusty::scene::Scene;

use super::with_api;

fn save_load(scene: &Scene, name: &str) -> Scene {
    let path = crate::temp::dir().join(name).to_string_lossy().into_owned();
    scene.save_to_file(&path).unwrap();
    let mut loaded = Scene::new();
    loaded.load_from_file(&path).unwrap();
    loaded
}

#[test]
fn layer_names_set_from_lua_and_persist() {
    let scene = RefCell::new(Scene::new());
    with_api(&scene, |lua| {
        lua.load(
            r#"
            Layers.SetName(8, "  Smoke ")
            assert(Layers.GetName(8) == "Smoke", "names are trimmed")
            assert(Layers.NameToIndex("Smoke") == 8)
            Layers.SetName(0, "Renamed")
            assert(Layers.GetName(0) == "Default", "slot 0 is fixed")
            Layers.SetName(9, "Gone")
            Layers.SetName(9, "")
            assert(Layers.GetName(9) == "Layer 9", "a blank name clears the slot")
            "#,
        )
        .exec()
        .unwrap();
        let err = lua.load("Layers.SetName(32, 'X')").exec().unwrap_err();
        assert!(err.to_string().contains("out of range"), "{err}");
    });
    let loaded = save_load(&scene.borrow(), "rusty_827_names.scene");
    assert_eq!(loaded.layers.index_of("Smoke"), Some(8));
}

#[test]
fn ignore_layer_collision_is_symmetric_and_persists() {
    let scene = RefCell::new(Scene::new());
    with_api(&scene, |lua| {
        lua.load(
            r#"
            assert(Physics.GetIgnoreLayerCollision(3, 5) == false, "all collide by default")
            Physics.IgnoreLayerCollision(3, 5)
            assert(Physics.GetIgnoreLayerCollision(5, 3), "the matrix is symmetric")
            Physics.IgnoreLayerCollision(7, 7, true)
            Physics.IgnoreLayerCollision(4, 6, true)
            Physics.IgnoreLayerCollision(6, 4, false)
            assert(not Physics.GetIgnoreLayerCollision(4, 6), "false re-enables the pair")
            "#,
        )
        .exec()
        .unwrap();
        let err = lua
            .load("Physics.IgnoreLayerCollision(0, 40)")
            .exec()
            .unwrap_err();
        assert!(err.to_string().contains("out of range"), "{err}");
    });
    let loaded = save_load(&scene.borrow(), "rusty_827_matrix.scene");
    assert!(!loaded.collision_matrix.can_collide(5, 3));
    assert!(!loaded.collision_matrix.can_collide(7, 7));
    assert!(loaded.collision_matrix.can_collide(4, 6));
    assert!(loaded.collision_matrix.can_collide(3, 4));
}

#[test]
fn culling_mask_set_from_lua_and_persists() {
    let mut scene = Scene::new();
    let cam = scene.add_entity("Cam".to_string());
    scene
        .world
        .set_camera(cam, Some(CameraComponent::default()));
    let bare = scene.add_entity("Bare".to_string());
    let scene = RefCell::new(scene);
    with_api(&scene, |lua| {
        let code = format!(
            r#"
            assert(Camera.GetCullingMask({cam}) == 0xFFFFFFFF, "a camera draws every layer")
            Camera.SetCullingMask({cam}, ~(1 << 8))
            assert(Camera.GetCullingMask({cam}) == 0xFFFFFEFF, "kept to 32 bits")
            assert(Camera.GetCullingMask({bare}) == nil)
            Camera.SetCullingMask({bare}, 1)
            Camera.SetCullingMask({cam}, 1 << 8)
            "#
        );
        lua.load(code).exec().unwrap();
    });
    assert!(scene.borrow().world.camera(bare).is_none());
    let loaded = save_load(&scene.borrow(), "rusty_827_mask.scene");
    assert_eq!(loaded.world.camera(cam).unwrap().culling_mask, 1 << 8);
}
