use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::scripting::instances;
use rusty_scene::Scene;

pub fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(scene
            .get(lua)?
            .borrow()
            .world
            .name(id.read())
            .map(|n| n.clone()))
    });
    put(table, "GetName", f)?;
    let f = lua.create_function(move |lua, (id, name): (EntityArg, String)| {
        let id = id.live("Scene", "SetName")?;
        Ok(scene.get(lua)?.borrow_mut().world.set_name(id, name))
    });
    put(table, "SetName", f)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(scene.get(lua)?.borrow().world.parent_id(id.read()))
    });
    put(table, "GetParent", f)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(scene.get(lua)?.borrow().world.children(id.read()))
    });
    put(table, "GetChildren", f)?;
    let f = lua.create_function(move |lua, (id, path): (EntityArg, String)| {
        Ok(scene.get(lua)?.borrow().world.find_child(id.read(), &path))
    });
    put(table, "FindChild", f)?;
    register_flags(lua, table, scene)?;
    let f = lua.create_function(|lua, (id, name): (EntityArg, String)| {
        instances::find(lua, id.read(), &name)
    });
    put(table, "GetScript", f)
}

fn register_flags(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    let f = lua.create_function(move |lua, (id, on): (EntityArg, bool)| {
        let id = id.live("Scene", "SetActive")?;
        Ok(scene.get(lua)?.borrow_mut().world.set_active(id, on))
    });
    put(table, "SetActive", f)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        // lint: allow(active-self) `IsActive` is Unity's `activeSelf`
        Ok(scene.get(lua)?.borrow().world.is_active_self(id.read()))
    });
    put(table, "IsActive", f)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(scene
            .get(lua)?
            .borrow()
            .world
            .is_active_in_hierarchy(id.read()))
    });
    put(table, "IsActiveInHierarchy", f)?;
    let f = lua.create_function(move |lua, (id, on): (EntityArg, bool)| {
        let id = id.live("Scene", "SetStatic")?;
        Ok(scene.get(lua)?.borrow_mut().world.set_static(id, on))
    });
    put(table, "SetStatic", f)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(scene.get(lua)?.borrow().world.is_static(id.read()))
    });
    put(table, "IsStatic", f)
}
