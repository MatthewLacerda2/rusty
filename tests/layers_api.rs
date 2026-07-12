//! Integration coverage for the `Layers` scripting namespace (issue #350): an
//! entity's layer index reads/writes through the scene, and name resolution goes
//! through the project's shared registry (layer 0 is always `"Default"`).

use std::cell::RefCell;

use mlua::Lua;
use rusty::scene::Scene;

fn scene_with_entity() -> (RefCell<Scene>, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Thing".to_string());
    (RefCell::new(scene), id)
}

#[test]
fn layer_index_round_trips_and_defaults_to_zero() {
    let lua = Lua::new();
    let (scene, id) = scene_with_entity();

    lua.scope(|scope| {
        rusty::api::layers::register(&lua, scope, &scene).unwrap();

        let layer: u8 = lua
            .load(format!("return Layers.GetLayer({id})"))
            .eval()
            .unwrap();
        assert_eq!(layer, 0, "a fresh entity sits on the Default layer");

        lua.load(format!("Layers.SetLayer({id}, 5)"))
            .exec()
            .unwrap();
        let layer: u8 = lua
            .load(format!("return Layers.GetLayer({id})"))
            .eval()
            .unwrap();
        assert_eq!(layer, 5);
        assert_eq!(scene.borrow().world.layer(id), 5);
        Ok(())
    })
    .unwrap();
}

#[test]
fn missing_entity_reads_zero_and_set_is_a_noop() {
    let lua = Lua::new();
    let (scene, _) = scene_with_entity();

    lua.scope(|scope| {
        rusty::api::layers::register(&lua, scope, &scene).unwrap();
        let layer: u8 = lua.load("return Layers.GetLayer(999)").eval().unwrap();
        assert_eq!(layer, 0);
        lua.load("Layers.SetLayer(999, 3)").exec().unwrap();
        Ok(())
    })
    .unwrap();
}

#[test]
fn name_resolution_mirrors_the_registry() {
    let lua = Lua::new();
    let (scene, _) = scene_with_entity();

    lua.scope(|scope| {
        rusty::api::layers::register(&lua, scope, &scene).unwrap();

        let name: String = lua.load("return Layers.GetName(0)").eval().unwrap();
        assert_eq!(name, "Default");

        let index: Option<u8> = lua
            .load("return Layers.NameToIndex('Default')")
            .eval()
            .unwrap();
        assert_eq!(index, Some(0));

        // Unity's LayerMask.NameToLayer contract: an unknown name is nil, not 0.
        let index: Option<u8> = lua
            .load("return Layers.NameToIndex('NoSuchLayer')")
            .eval()
            .unwrap();
        assert_eq!(index, None);
        Ok(())
    })
    .unwrap();
}
