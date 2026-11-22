//! `OverrideArea` bakes the colliders as an area (`1` = `NotWalkable`); `IgnoreFromBuild` leaves them out. Getters return a neutral default without a modifier; setters are then no-ops.

use std::cell::RefCell;

use mlua::{Lua, Table};

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_components::NavMeshModifierComponent as Modifier;
use rusty_edit::authoring::nav::object_modifier as ops;
use rusty_scene::Scene;

const NS: &str = "NavMeshModifier";

pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    register_flags(lua, &t, scene)?;
    register_area(lua, &t, scene)?;
    lua.globals().set(NS, t).map_err(|e| e.to_string())
}

fn get(scene: &RefCell<Scene>, id: u32) -> Option<Modifier> {
    scene
        .borrow()
        .world
        .nav_mesh_modifier(id)
        .map(|m| m.clone())
}

/// Apply a shared op to `id`'s modifier, if it has one. A write that changes
/// nothing (a script repeating a value every tick) never borrows the component
/// mutably, so it stamps no change and Play's navmesh check stays skipped.
fn write(scene: &RefCell<Scene>, id: u32, op: impl FnOnce(&mut Modifier)) {
    let Some(old) = get(scene, id) else { return };
    let mut new = old.clone();
    op(&mut new);
    if new == old {
        return;
    }
    if let Some(mut m) = scene.borrow_mut().world.nav_mesh_modifier_mut(id) {
        *m = new;
    }
}

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
