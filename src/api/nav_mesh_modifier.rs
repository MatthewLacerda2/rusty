//! src/api/nav_mesh_modifier.rs — `NavMeshModifier` namespace (#1959).
//!
//! Unity's `NavMeshModifier`: how the navmesh bake reads an object's colliders, and
//! by default its children's. `OverrideArea` bakes them as an area (`1`,
//! `NotWalkable`, takes a tabletop off the navmesh while the table still blocks the
//! floor under it); `IgnoreFromBuild` leaves them out entirely. Every setter routes
//! through the shared `scene::authoring::nav::object_modifier` ops the inspector card
//! and `Component.Set` use; the next navmesh sync rebakes the covered colliders'
//! cells. Getters return a neutral default without a modifier; setters are then
//! no-ops. Attach one with `Scene.AddComponent(id, "NavMeshModifier")`.

use std::cell::RefCell;

use mlua::{Lua, Table};

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::components::NavMeshModifierComponent as Modifier;
use crate::scene::authoring::nav::object_modifier as ops;
use crate::scene::Scene;

const NS: &str = "NavMeshModifier";

/// Register the `NavMeshModifier` namespace onto `lua`.
pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    register_flags(lua, &t, scene)?;
    register_area(lua, &t, scene)?;
    lua.globals().set(NS, t).map_err(|e| e.to_string())
}

/// `id`'s modifier, cloned, or `None` without one.
fn get(scene: &RefCell<Scene>, id: u32) -> Option<Modifier> {
    scene
        .borrow()
        .world
        .nav_mesh_modifier(id)
        .map(|m| m.clone())
}

/// Apply a shared op to `id`'s modifier, if it has one.
fn write(scene: &RefCell<Scene>, id: u32, op: impl FnOnce(&mut Modifier)) {
    if let Some(mut m) = scene.borrow_mut().world.nav_mesh_modifier_mut(id) {
        op(&mut m);
    }
}

/// `Active`, `IgnoreFromBuild` and `ApplyToChildren` (`false` without a modifier).
fn register_flags(lua: &Lua, t: &Table, scene: ApiCell<Scene>) -> Reg {
    type Flag = (fn(&Modifier) -> bool, fn(&mut Modifier, bool));
    let flags: [(&str, Flag); 3] = [
        ("Active", (|m| m.active, ops::set_active)),
        (
            "IgnoreFromBuild",
            (|m| m.ignore_from_build, ops::set_ignore_from_build),
        ),
        (
            "ApplyToChildren",
            (|m| m.apply_to_children, ops::set_apply_to_children),
        ),
    ];
    for (name, (read, set)) in flags {
        let f = lua.create_function(move |lua, id: EntityArg| {
            Ok(get(&*scene.get(lua)?, id.read()).is_some_and(|m| read(&m)))
        });
        put(t, &format!("Get{name}"), f)?;
        let verb = format!("Set{name}");
        let f = lua.create_function(move |lua, (id, on): (EntityArg, bool)| {
            let id = id.live(NS, &verb)?;
            write(&*scene.get(lua)?, id, |m| set(m, on));
            Ok(())
        });
        put(t, &format!("Set{name}"), f)?;
    }
    Ok(())
}

/// `OverrideArea`: an area id, or `nil` for none (also without a modifier).
fn register_area(lua: &Lua, t: &Table, scene: ApiCell<Scene>) -> Reg {
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(get(&*scene.get(lua)?, id.read()).and_then(|m| m.override_area))
    });
    put(t, "GetOverrideArea", f)?;
    let f = lua.create_function(move |lua, (id, area): (EntityArg, Option<i64>)| {
        let id = id.live(NS, "SetOverrideArea")?;
        write(&*scene.get(lua)?, id, |m| ops::set_override_area(m, area));
        Ok(())
    });
    put(t, "SetOverrideArea", f)
}

#[cfg(test)]
#[path = "nav_mesh_modifier_tests.rs"]
mod tests;
