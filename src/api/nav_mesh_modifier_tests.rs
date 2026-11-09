//! The `NavMeshModifier` namespace (#1959), through Lua: setters route through the
//! shared ops, getters read back, and an entity without a modifier reads neutral
//! defaults and is never given one.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;

use super::register;
use crate::api::bind;
use crate::components::NavMeshModifierComponent;
use crate::scene::Scene;

#[test]
fn setters_write_through_the_shared_ops_and_getters_read_back() {
    let mut scene = Scene::new();
    let id = scene.add_entity("table".to_string());
    let m = NavMeshModifierComponent::default();
    scene.world.set_nav_mesh_modifier(id, Some(m));
    let bare = scene.add_entity("bare".to_string());
    let scene = Rc::new(RefCell::new(scene));
    let lua = Lua::new();
    crate::api::attempt(|| {
        register(&lua, bind(&lua, &scene)).unwrap();
        lua.globals().set("id", id)?;
        lua.globals().set("bare", bare)?;
        lua.load(
            "NavMeshModifier.SetOverrideArea(id, 1)
             NavMeshModifier.SetOverrideArea(id, 99)
             NavMeshModifier.SetIgnoreFromBuild(id, true)
             NavMeshModifier.SetApplyToChildren(id, false)
             NavMeshModifier.SetActive(bare, true)",
        )
        .exec()?;
        let read: (Option<u8>, bool, bool, bool) = lua
            .load(
                "return NavMeshModifier.GetOverrideArea(id), NavMeshModifier.GetIgnoreFromBuild(id),
                 NavMeshModifier.GetApplyToChildren(id), NavMeshModifier.GetActive(id)",
            )
            .eval()?;
        assert_eq!(read, (Some(1), true, false, true), "99 is out of range");
        lua.load("NavMeshModifier.SetOverrideArea(id, nil)")
            .exec()?;
        let none: (Option<u8>, bool) = lua
            .load("return NavMeshModifier.GetOverrideArea(id), NavMeshModifier.GetActive(bare)")
            .eval()?;
        assert_eq!(
            none,
            (None, false),
            "nil clears; a bare entity reads neutral"
        );
        Ok(())
    })
    .unwrap();
    let scene = scene.borrow();
    assert!(
        scene.world.nav_mesh_modifier(bare).is_none(),
        "never attached"
    );
}

#[test]
fn a_setter_repeating_the_value_stamps_no_change() {
    let mut scene = Scene::new();
    let id = scene.add_entity("table".to_string());
    let m = NavMeshModifierComponent::default();
    scene.world.set_nav_mesh_modifier(id, Some(m));
    let mut cursor = scene.world.change_cursor::<NavMeshModifierComponent>();
    let scene = Rc::new(RefCell::new(scene));
    let lua = Lua::new();
    crate::api::attempt(|| {
        register(&lua, bind(&lua, &scene)).unwrap();
        lua.globals().set("id", id)?;
        lua.load("NavMeshModifier.SetActive(id, true) NavMeshModifier.SetOverrideArea(id, nil)")
            .exec()
    })
    .unwrap();
    let changes = scene.borrow().world.changes(&mut cursor);
    assert_eq!(changes, crate::ecs::Changes::Ids(vec![]));
}
