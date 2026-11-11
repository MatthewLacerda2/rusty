//! src/api/physics/body.rs — `Physics.GetMass` / `SetMass` / `GetUseGravity` /
//! `SetUseGravity` / `IsTrigger` / `SetTrigger` (#1365, #1368), and the drag
//! pair `GetDrag` / `SetDrag` / `GetAngularDrag` / `SetAngularDrag` (#2062).
//!
//! The body's authorable switches, over the same `scene::authoring` ops the
//! inspector's Rigidbody and Collider cards write through, so a script's mass is
//! clamped exactly as the card's is. Unity: `Rigidbody.mass`, `Rigidbody.useGravity`,
//! `Rigidbody.drag`, `Rigidbody.angularDrag` and `Collider.isTrigger`. Getters answer a neutral default without the
//! component; setters are then no-ops.

use mlua::Table;

use super::super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::components::RigidBodyComponent;
use crate::scene::authoring::{physics::collider as collider_ops, physics::rigidbody as rb_ops};
use crate::scene::Scene;

/// Put a getter/setter pair over one `f32` Rigidbody field: the getter reads it
/// (`0` without a Rigidbody), the setter writes it through `set` (a no-op then).
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

/// Register the mass, drag, gravity and trigger accessors onto the `Physics` table.
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
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(scene
            .get(lua)?
            .borrow()
            .world
            .rigidbody(id.read())
            .map_or(0.0, |rb| rb.mass))
    });
    put(table, "GetMass", f)?;
    let f = lua.create_function(move |lua, (id, mass): (EntityArg, f32)| {
        let id = id.live("Physics", "SetMass")?;
        if let Some(mut rb) = scene.get(lua)?.borrow_mut().world.rigidbody_mut(id) {
            rb_ops::set_mass(&mut rb, mass);
        }
        Ok(())
    });
    put(table, "SetMass", f)?;
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
