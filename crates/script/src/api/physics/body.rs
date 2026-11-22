use mlua::Table;

use super::super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_components::RigidBodyComponent;
use rusty_edit::authoring::{physics::collider as collider_ops, physics::rigidbody as rb_ops};
use rusty_scene::Scene;

/// `set` is the inspector's `rb_ops` setter, so a script's value is clamped as the card's is.
fn number_pair(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    (get_name, set_name): (&'static str, &'static str),
    get: fn(&RigidBodyComponent) -> f32,
    set: fn(&mut RigidBodyComponent, f32),
) -> Reg {
    let f = lua.create_function(move |lua, id: EntityArg| {
        let scene = &*scene.get(lua)?;
        let world = &scene.borrow().world;
        Ok(world.rigidbody(id.read()).map_or(0.0, |rb| get(&rb)))
    });
    put(table, get_name, f)?;
    let f = lua.create_function(move |lua, (id, v): (EntityArg, f32)| {
        let id = id.live("Physics", set_name)?;
        if let Some(mut rb) = scene.get(lua)?.borrow_mut().world.rigidbody_mut(id) {
            set(&mut rb, v);
        }
        Ok(())
    });
    put(table, set_name, f)
}

pub(super) fn register(lua: &mlua::Lua, table: &Table, scene: ApiCell<Scene>) -> Reg {
    let names = ("GetDrag", "SetDrag");
    number_pair(lua, table, scene, names, |rb| rb.drag, rb_ops::set_drag)?;
    let names = ("GetAngularDrag", "SetAngularDrag");
    number_pair(
        lua,
        table,
        scene,
        names,
        |rb| rb.angular_drag,
        rb_ops::set_angular_drag,
    )?;
    number_pair(
        lua,
        table,
        scene,
        ("GetMass", "SetMass"),
        |rb| rb.mass,
        rb_ops::set_mass,
    )?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        let scene = &*scene.get(lua)?;
        let world = &scene.borrow().world;
        Ok(world.rigidbody(id.read()).is_some_and(|rb| rb.use_gravity))
    });
    put(table, "GetUseGravity", f)?;
    let f = lua.create_function(move |lua, (id, on): (EntityArg, bool)| {
        let id = id.live("Physics", "SetUseGravity")?;
        if let Some(mut rb) = scene.get(lua)?.borrow_mut().world.rigidbody_mut(id) {
            rb_ops::set_use_gravity(&mut rb, on);
        }
        Ok(())
    });
    put(table, "SetUseGravity", f)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        let scene = &*scene.get(lua)?;
        let world = &scene.borrow().world;
        Ok(world.collider(id.read()).is_some_and(|c| c.is_trigger))
    });
    put(table, "IsTrigger", f)?;
    let f = lua.create_function(move |lua, (id, on): (EntityArg, bool)| {
        let id = id.live("Physics", "SetTrigger")?;
        if let Some(mut c) = scene.get(lua)?.borrow_mut().world.collider_mut(id) {
            collider_ops::set_trigger(&mut c, on);
        }
        Ok(())
    });
    put(table, "SetTrigger", f)
}
