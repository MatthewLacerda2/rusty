//! src/api/nav/path_state.rs — `NavMeshAgent`'s path state (#458): `HasPath`,
//! `PathPending` (#2087), `RemainingDistance`, `GetPathStatus`, `GetPath`, `Warp` and `ResetPath`, read from
//! and written to the agent's cached path (Unity's `NavMeshAgent` path members).

use glam::Vec3;
use mlua::Table;

use super::super::{put, Reg};
use super::query::path_table;
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::navigation::{remaining_corners, remaining_distance, reset_path, NavigationGraph};
use crate::scene::Scene;

/// The entity's world position, where the agent steers from (the origin for a dead
/// id).
fn position(scene: &Scene, id: u32) -> Vec3 {
    scene.world.world_position(id)
}

/// The read-only path state: `HasPath`, `PathPending`, `RemainingDistance`, then
/// [`register_path`].
pub(super) fn register_reads(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    nav: ApiCell<NavigationGraph>,
) -> Reg {
    put(
        table,
        "HasPath",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            Ok(scene
                .world
                .nav_agent(id.read())
                .is_some_and(|a| !a.cached_path.is_empty()))
        }),
    )?;
    put(
        table,
        "PathPending",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let (id, scene) = (id.read(), scene.borrow());
            let at = position(&scene, id);
            let nav = &*nav.get(lua)?;
            let nav = nav.borrow();
            Ok(scene
                .world
                .nav_agent(id)
                .is_some_and(|a| nav.path_pending(&a, at)))
        }),
    )?;
    put(
        table,
        "RemainingDistance",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let id = id.read();
            let scene = scene.borrow();
            let at = position(&scene, id);
            Ok(scene
                .world
                .nav_agent(id)
                .map_or(f32::INFINITY, |a| remaining_distance(&a, at)))
        }),
    )?;
    register_path(lua, table, scene)
}

/// `GetPathStatus` and `GetPath`: the path itself.
fn register_path(lua: &mlua::Lua, table: &Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetPathStatus",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let status = scene.world.nav_agent(id.read()).map(|a| a.path_status);
            Ok(status.unwrap_or_default().name())
        }),
    )?;
    put(
        table,
        "GetPath",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let id = id.read();
            let scene = scene.borrow();
            let at = position(&scene, id);
            let (status, corners) = scene
                .world
                .nav_agent(id)
                .map(|a| (a.path_status, remaining_corners(&a, at)))
                .unwrap_or_default();
            path_table(lua, status, &corners)
        }),
    )
}

/// `Warp` and `ResetPath`: the two ways a script interrupts the path.
pub(super) fn register_writes(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    nav: ApiCell<NavigationGraph>,
) -> Reg {
    put(
        table,
        "Warp",
        lua.create_function(move |lua, (id, x, y, z): (EntityArg, f32, f32, f32)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("NavMeshAgent", "Warp")?;
            let mut scene = scene.borrow_mut();
            let landed = match scene.world.nav_agent_mut(id) {
                Some(mut a) => nav
                    .get(lua)?
                    .borrow()
                    .warp_agent(&mut a, Vec3::new(x, y, z)),
                None => None,
            };
            if let (Some(p), Some(mut t)) = (landed, scene.world.transform_mut(id)) {
                t.position = p;
            }
            Ok(landed.is_some())
        }),
    )?;
    put(
        table,
        "ResetPath",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let id = id.live("NavMeshAgent", "ResetPath")?;
            let mut scene = scene.borrow_mut();
            let at = position(&scene, id);
            if let Some(mut a) = scene.world.nav_agent_mut(id) {
                reset_path(&mut a, at);
            }
            Ok(())
        }),
    )
}
