//! crates/script/src/api/offmesh_link.rs — `OffMeshLink` namespace (#462).
//!
//! Unity's `OffMeshLink` / `NavMeshLink`: an authored link between two walkable
//! points (a ladder, a window, a gap) — its two ends (offsets from the entity's
//! Transform), whether it runs both ways, its path cost and area (#460), and whether
//! it is active;
//! plus `IsConnected`, whether both ends found the navmesh at the last rebake.
//! Every setter routes through the shared `scene::authoring::nav::offmesh_link` ops the
//! inspector card uses. Getters return a neutral default without a link; setters
//! are then no-ops. Attach one with `Scene.AddComponent(id, "OffMeshLink")`.

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Table};

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_components::OffMeshLinkComponent as Link;
use rusty_navigation::NavigationGraph;
use rusty_scene::authoring::nav::offmesh_link as ops;
use rusty_scene::Scene;

/// Register the `OffMeshLink` namespace onto `lua`.
pub fn register(lua: &Lua, scene: ApiCell<Scene>, nav: ApiCell<NavigationGraph>) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    register_ends(lua, &t, scene)?;
    register_options(lua, &t, scene)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(nav.get(lua)?.borrow().link_connected(id.read()))
    });
    put(&t, "IsConnected", f)?;
    lua.globals()
        .set("OffMeshLink", t)
        .map_err(|e| e.to_string())
}

/// `id`'s link, cloned, or `None` without one.
fn get(scene: &RefCell<Scene>, id: u32) -> Option<Link> {
    scene.borrow().world.offmesh_link(id).map(|l| l.clone())
}

/// Apply a shared op to `id`'s link, if it has one.
fn write(scene: &RefCell<Scene>, id: u32, op: impl FnOnce(&mut Link)) {
    if let Some(mut l) = scene.borrow_mut().world.offmesh_link_mut(id) {
        op(&mut l);
    }
}

/// `GetStart` / `SetStart` and `GetEnd` / `SetEnd`, as `x, y, z` local offsets.
fn register_ends(lua: &mlua::Lua, t: &Table, scene: ApiCell<Scene>) -> Reg {
    type End = (fn(&Link) -> Vec3, fn(&mut Link, Vec3));
    let ends: [(&str, End); 2] = [
        ("Start", (|l| l.start, ops::set_start)),
        ("End", (|l| l.end, ops::set_end)),
    ];
    for (name, (read, set)) in ends {
        let f = lua.create_function(move |lua, id: EntityArg| {
            let v = get(&*scene.get(lua)?, id.read()).map_or(Vec3::ZERO, |l| read(&l));
            Ok((v.x, v.y, v.z))
        });
        put(t, &format!("Get{name}"), f)?;
        let verb = format!("Set{name}");
        let f = lua.create_function(move |lua, (id, x, y, z): (EntityArg, f32, f32, f32)| {
            let id = id.live("OffMeshLink", &verb)?;
            write(&*scene.get(lua)?, id, |l| set(l, Vec3::new(x, y, z)));
            Ok(())
        });
        put(t, &format!("Set{name}"), f)?;
    }
    Ok(())
}

/// `Active`, `Bidirectional`, `Area` (#460) and `Cost` (negative: the link's length
/// at its area's cost).
fn register_options(lua: &mlua::Lua, t: &Table, scene: ApiCell<Scene>) -> Reg {
    type Flag = (fn(&Link) -> bool, fn(&mut Link, bool));
    let flags: [(&str, Flag); 2] = [
        ("Active", (|l| l.active, ops::set_active)),
        (
            "Bidirectional",
            (|l| l.bidirectional, ops::set_bidirectional),
        ),
    ];
    for (name, (read, set)) in flags {
        let f = lua.create_function(move |lua, id: EntityArg| {
            Ok(get(&*scene.get(lua)?, id.read()).is_some_and(|l| read(&l)))
        });
        put(t, &format!("Get{name}"), f)?;
        let verb = format!("Set{name}");
        let f = lua.create_function(move |lua, (id, v): (EntityArg, bool)| {
            let id = id.live("OffMeshLink", &verb)?;
            write(&*scene.get(lua)?, id, |l| set(l, v));
            Ok(())
        });
        put(t, &format!("Set{name}"), f)?;
    }
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(get(&*scene.get(lua)?, id.read()).map_or(0, |l| l.area))
    });
    put(t, "GetArea", f)?;
    let f = lua.create_function(move |lua, (id, area): (EntityArg, i64)| {
        let id = id.live("OffMeshLink", "SetArea")?;
        write(&*scene.get(lua)?, id, |l| ops::set_area(l, area));
        Ok(())
    });
    put(t, "SetArea", f)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        Ok(get(&*scene.get(lua)?, id.read()).map_or(-1.0, |l| l.cost_override))
    });
    put(t, "GetCost", f)?;
    let f = lua.create_function(move |lua, (id, cost): (EntityArg, f32)| {
        let id = id.live("OffMeshLink", "SetCost")?;
        write(&*scene.get(lua)?, id, |l| ops::set_cost_override(l, cost));
        Ok(())
    });
    put(t, "SetCost", f)
}

#[cfg(test)]
#[path = "offmesh_link_tests.rs"]
mod tests;
