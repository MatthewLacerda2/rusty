//! crates/script/src/api/nav/offmesh.rs — off-mesh links on the `NavMeshAgent` and `Navigation`
//! namespaces (#462).
//!
//! `NavMeshAgent`: `IsOnOffMeshLink`, `GetCurrentOffMeshLink`, `CompleteOffMeshLink`
//! and `Get/SetAutoTraverseOffMeshLink` (Unity's agent link members). `Navigation`:
//! `GetOffMeshLinks`, every link in the navmesh, and the generation settings.
//!
//! A link is `{type = "manual"|"drop"|"jump", startPos = {x,y,z}, endPos = {x,y,z},
//! owner = id|nil}` (Unity's `OffMeshLinkData`); `GetOffMeshLinks` adds
//! `bidirectional`.

use glam::Vec3;
use mlua::{Lua, Table};

use super::super::{put, Reg};
use super::settings::set_and_rebake;
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use rusty_components::OffMeshLinkData;
use rusty_navigation::{complete_off_mesh_link, NavMeshSettings, NavigationGraph};
use rusty_scene::authoring::nav::agent as nav_ops;
use rusty_scene::Scene;

fn point(lua: &Lua, p: Vec3) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("x", p.x)?;
    t.set("y", p.y)?;
    t.set("z", p.z)?;
    Ok(t)
}

/// A link as its Lua table.
fn link_table(lua: &Lua, l: &OffMeshLinkData) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("type", l.kind.name())?;
    t.set("startPos", point(lua, l.start)?)?;
    t.set("endPos", point(lua, l.end)?)?;
    t.set("owner", l.owner)?;
    Ok(t)
}

/// The agent's link state and the script's two levers on it.
pub(super) fn register_agent(lua: &mlua::Lua, t: &Table, scene: ApiCell<Scene>) -> Reg {
    let current = move |lua: &mlua::Lua, id: u32| -> mlua::Result<_> {
        let scene = scene.get(lua)?;
        let link = scene
            .borrow()
            .world
            .nav_agent(id)
            .and_then(|a| a.off_mesh_link);
        Ok(link)
    };
    let f = lua.create_function(move |lua, id: EntityArg| Ok(current(lua, id.read())?.is_some()));
    put(t, "IsOnOffMeshLink", f)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        current(lua, id.read())?
            .map(|l| link_table(lua, &l))
            .transpose()
    });
    put(t, "GetCurrentOffMeshLink", f)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        let scene = &*scene.get(lua)?;
        let id = id.live("NavMeshAgent", "CompleteOffMeshLink")?;
        let mut scene = scene.borrow_mut();
        let end = match scene.world.nav_agent_mut(id) {
            Some(mut a) => complete_off_mesh_link(&mut a),
            None => None,
        };
        if let (Some(p), Some(mut tr)) = (end, scene.world.transform_mut(id)) {
            tr.position = p;
        }
        Ok(end.is_some())
    });
    put(t, "CompleteOffMeshLink", f)?;
    let f = lua.create_function(move |lua, id: EntityArg| {
        let scene = &*scene.get(lua)?;
        let auto = scene
            .borrow()
            .world
            .nav_agent(id.read())
            .map(|a| a.auto_traverse_off_mesh_link);
        Ok(auto.unwrap_or(false))
    });
    put(t, "GetAutoTraverseOffMeshLink", f)?;
    let f = lua.create_function(move |lua, (id, auto): (EntityArg, bool)| {
        let id = id.live("NavMeshAgent", "SetAutoTraverseOffMeshLink")?;
        if let Some(mut a) = scene.get(lua)?.borrow_mut().world.nav_agent_mut(id) {
            nav_ops::set_auto_traverse_off_mesh_link(&mut a, auto);
        }
        Ok(())
    });
    put(t, "SetAutoTraverseOffMeshLink", f)
}

/// `Navigation.GetOffMeshLinks` and the generation settings' getters and setters
/// (`DropHeight`, `JumpDistance`, `JumpHeight`, `LinkSpacing`), which re-bake.
pub(super) fn register_navigation(
    lua: &mlua::Lua,
    t: &Table,
    scene: ApiCell<Scene>,
    nav: ApiCell<NavigationGraph>,
) -> Reg {
    let f = lua.create_function(move |lua, ()| {
        let list = lua.create_table()?;
        for (i, l) in nav.get(lua)?.borrow().offmesh_links().enumerate() {
            let row = link_table(lua, &l.data(true))?;
            row.set("bidirectional", l.bidirectional)?;
            list.set(i + 1, row)?;
        }
        Ok(list)
    });
    put(t, "GetOffMeshLinks", f)?;
    type Knob = (fn(&NavMeshSettings) -> f32, fn(&mut NavMeshSettings, f32));
    let knobs: [(&str, Knob); 4] = [
        ("DropHeight", (|s| s.drop_height, |s, v| s.drop_height = v)),
        (
            "JumpDistance",
            (|s| s.jump_distance, |s, v| s.jump_distance = v),
        ),
        ("JumpHeight", (|s| s.jump_height, |s, v| s.jump_height = v)),
        (
            "LinkSpacing",
            (|s| s.link_spacing, |s, v| s.link_spacing = v),
        ),
    ];
    for (name, (read, write)) in knobs {
        let f =
            lua.create_function(move |lua, ()| Ok(read(&scene.get(lua)?.borrow().nav_settings)));
        put(t, &format!("Get{name}"), f)?;
        let f = lua.create_function(move |lua, v: f32| {
            set_and_rebake(&*scene.get(lua)?, &*nav.get(lua)?, |s| write(s, v))
        });
        put(t, &format!("Set{name}"), f)?;
    }
    Ok(())
}
