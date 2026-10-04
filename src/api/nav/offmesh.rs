//! src/api/nav/offmesh.rs — off-mesh links on the `NavMeshAgent` and `Navigation`
//! namespaces (#462).
//!
//! `NavMeshAgent`: `IsOnOffMeshLink`, `GetCurrentOffMeshLink`, `CompleteOffMeshLink`
//! and `Get/SetAutoTraverseOffMeshLink` (Unity's agent link members). `Navigation`:
//! `GetOffMeshLinks`, every link in the navmesh, and the generation settings.
//!
//! A link is `{type = "manual"|"drop"|"jump", startPos = {x,y,z}, endPos = {x,y,z},
//! owner = id|nil}` (Unity's `OffMeshLinkData`); `GetOffMeshLinks` adds
//! `bidirectional`.

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Table};

use super::super::{put, Reg};
use super::set_and_rebake;
use crate::components::OffMeshLinkData;
use crate::navigation::{complete_off_mesh_link, NavMeshSettings, NavigationGraph};
use crate::scene::authoring::nav_agent as nav_ops;
use crate::scene::Scene;

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
    t.set("type", l.kind.as_str())?;
    t.set("startPos", point(lua, l.start)?)?;
    t.set("endPos", point(lua, l.end)?)?;
    t.set("owner", l.owner)?;
    Ok(t)
}

/// The agent's link state and the script's two levers on it.
pub(super) fn register_agent<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    t: &Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let current = move |id: u32| scene.borrow().world.nav_agent(id)?.off_mesh_link;
    let f = scope.create_function(move |_, id: u32| Ok(current(id).is_some()));
    put(t, "IsOnOffMeshLink", f)?;
    let f = scope
        .create_function(move |lua, id: u32| current(id).map(|l| link_table(lua, &l)).transpose());
    put(t, "GetCurrentOffMeshLink", f)?;
    let f = scope.create_function(move |_, id: u32| {
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
    let f = scope.create_function(move |_, id: u32| {
        let auto = scene
            .borrow()
            .world
            .nav_agent(id)
            .map(|a| a.auto_traverse_off_mesh_link);
        Ok(auto.unwrap_or(false))
    });
    put(t, "GetAutoTraverseOffMeshLink", f)?;
    let f = scope.create_function(move |_, (id, auto): (u32, bool)| {
        if let Some(mut a) = scene.borrow_mut().world.nav_agent_mut(id) {
            nav_ops::set_auto_traverse_off_mesh_link(&mut a, auto);
        }
        Ok(())
    });
    put(t, "SetAutoTraverseOffMeshLink", f)
}

/// `Navigation.GetOffMeshLinks` and the generation settings' getters and setters
/// (`DropHeight`, `JumpDistance`, `JumpHeight`, `LinkSpacing`), which re-bake.
pub(super) fn register_navigation<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    t: &Table,
    scene: &'scope RefCell<Scene>,
    nav: &'scope RefCell<NavigationGraph>,
) -> Reg {
    let f = scope.create_function(move |lua, ()| {
        let list = lua.create_table()?;
        for (i, l) in nav.borrow().offmesh_links().enumerate() {
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
        let f = scope.create_function(move |_, ()| Ok(read(&scene.borrow().nav_settings)));
        put(t, &format!("Get{name}"), f)?;
        let f = scope.create_function(move |_, v: f32| set_and_rebake(scene, nav, |s| write(s, v)));
        put(t, &format!("Set{name}"), f)?;
    }
    Ok(())
}
