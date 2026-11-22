//! Getters return a neutral default without a link; setters are then no-ops.

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Table};

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_components::OffMeshLinkComponent as Link;
use rusty_edit::authoring::nav::offmesh_link as ops;
use rusty_navigation::NavigationGraph;
use rusty_scene::Scene;

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

fn get(scene: &RefCell<Scene>, id: u32) -> Option<Link> {
    scene.borrow().world.offmesh_link(id).map(|l| l.clone())
}

fn write(scene: &RefCell<Scene>, id: u32, op: impl FnOnce(&mut Link)) {
    if let Some(mut l) = scene.borrow_mut().world.offmesh_link_mut(id) {
        op(&mut l);
    }
}

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

/// `Cost` negative means the link's length at its area's cost.
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
