//! src/api/offmesh_link.rs — `OffMeshLink` namespace (#462).
//!
//! Unity's `OffMeshLink` / `NavMeshLink`: an authored link between two walkable
//! points (a ladder, a window, a gap) — its two ends (offsets from the entity's
//! Transform), whether it runs both ways, its path cost, and whether it is active;
//! plus `IsConnected`, whether both ends found the navmesh at the last rebake.
//! Every setter routes through the shared `scene::authoring::offmesh_link` ops the
//! inspector card uses. Getters return a neutral default without a link; setters
//! are then no-ops. Attach one with `Scene.AddComponent(id, "OffMeshLink")`.

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Table};

use super::{put, Reg};
use crate::components::OffMeshLinkComponent as Link;
use crate::navigation::NavigationGraph;
use crate::scene::authoring::offmesh_link as ops;
use crate::scene::Scene;

type SceneCell<'s> = &'s RefCell<Scene>;

/// Register the `OffMeshLink` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
    nav: &'scope RefCell<NavigationGraph>,
) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    register_ends(scope, &t, scene)?;
    register_options(scope, &t, scene)?;
    let f = scope.create_function(move |_, id: u32| Ok(nav.borrow().link_connected(id)));
    put(&t, "IsConnected", f)?;
    lua.globals()
        .set("OffMeshLink", t)
        .map_err(|e| e.to_string())
}

/// `id`'s link, cloned, or `None` without one.
fn get(scene: SceneCell, id: u32) -> Option<Link> {
    scene.borrow().world.offmesh_link(id).map(|l| l.clone())
}

/// Apply a shared op to `id`'s link, if it has one.
fn write(scene: SceneCell, id: u32, op: impl FnOnce(&mut Link)) {
    if let Some(mut l) = scene.borrow_mut().world.offmesh_link_mut(id) {
        op(&mut l);
    }
}

/// `GetStart` / `SetStart` and `GetEnd` / `SetEnd`, as `x, y, z` local offsets.
fn register_ends<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    t: &Table<'lua>,
    scene: SceneCell<'scope>,
) -> Reg {
    type End = (fn(&Link) -> Vec3, fn(&mut Link, Vec3));
    let ends: [(&str, End); 2] = [
        ("Start", (|l| l.start, ops::set_start)),
        ("End", (|l| l.end, ops::set_end)),
    ];
    for (name, (read, set)) in ends {
        let f = scope.create_function(move |_, id: u32| {
            let v = get(scene, id).map_or(Vec3::ZERO, |l| read(&l));
            Ok((v.x, v.y, v.z))
        });
        put(t, &format!("Get{name}"), f)?;
        let f = scope.create_function(move |_, (id, x, y, z): (u32, f32, f32, f32)| {
            write(scene, id, |l| set(l, Vec3::new(x, y, z)));
            Ok(())
        });
        put(t, &format!("Set{name}"), f)?;
    }
    Ok(())
}

/// `Active`, `Bidirectional` and `Cost` (negative: the link's length).
fn register_options<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    t: &Table<'lua>,
    scene: SceneCell<'scope>,
) -> Reg {
    type Flag = (fn(&Link) -> bool, fn(&mut Link, bool));
    let flags: [(&str, Flag); 2] = [
        ("Active", (|l| l.active, ops::set_active)),
        (
            "Bidirectional",
            (|l| l.bidirectional, ops::set_bidirectional),
        ),
    ];
    for (name, (read, set)) in flags {
        let f =
            scope.create_function(move |_, id: u32| Ok(get(scene, id).is_some_and(|l| read(&l))));
        put(t, &format!("Get{name}"), f)?;
        let f = scope.create_function(move |_, (id, v): (u32, bool)| {
            write(scene, id, |l| set(l, v));
            Ok(())
        });
        put(t, &format!("Set{name}"), f)?;
    }
    let f = scope
        .create_function(move |_, id: u32| Ok(get(scene, id).map_or(-1.0, |l| l.cost_override)));
    put(t, "GetCost", f)?;
    let f = scope.create_function(move |_, (id, cost): (u32, f32)| {
        write(scene, id, |l| ops::set_cost_override(l, cost));
        Ok(())
    });
    put(t, "SetCost", f)
}

#[cfg(test)]
#[path = "offmesh_link_tests.rs"]
mod tests;
