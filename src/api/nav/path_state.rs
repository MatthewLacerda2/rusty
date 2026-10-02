//! src/api/nav/path_state.rs — `NavMeshAgent`'s path state (#458): `HasPath`,
//! `RemainingDistance`, `GetPathStatus`, `GetPath`, `Warp` and `ResetPath`, read from
//! and written to the agent's cached path (Unity's `NavMeshAgent` path members).

use std::cell::RefCell;

use glam::Vec3;
use mlua::Table;

use super::super::{put, Reg};
use super::query::path_table;
use crate::navigation::{remaining_corners, remaining_distance, reset_path, NavigationGraph};
use crate::scene::Scene;

/// The entity's position, or the origin when it has no transform.
fn position(scene: &Scene, id: u32) -> Vec3 {
    scene
        .world
        .transform(id)
        .map(|t| t.position)
        .unwrap_or(Vec3::ZERO)
}

/// The read-only path state.
pub(super) fn register_reads<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "HasPath",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            Ok(scene
                .world
                .nav_agent(id)
                .is_some_and(|a| !a.cached_path.is_empty()))
        }),
    )?;
    put(
        table,
        "RemainingDistance",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            let at = position(&scene, id);
            Ok(scene
                .world
                .nav_agent(id)
                .map_or(f32::INFINITY, |a| remaining_distance(&a, at)))
        }),
    )?;
    put(
        table,
        "GetPathStatus",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            let status = scene.world.nav_agent(id).map(|a| a.path_status);
            Ok(status.unwrap_or_default().as_str())
        }),
    )?;
    put(
        table,
        "GetPath",
        scope.create_function(|lua, id: u32| {
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
pub(super) fn register_writes<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
    nav: &'scope RefCell<NavigationGraph>,
) -> Reg {
    put(
        table,
        "Warp",
        scope.create_function(|_, (id, x, y, z): (u32, f32, f32, f32)| {
            let mut scene = scene.borrow_mut();
            let landed = match scene.world.nav_agent_mut(id) {
                Some(mut a) => nav.borrow().warp_agent(&mut a, Vec3::new(x, y, z)),
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
        scope.create_function(|_, id: u32| {
            let mut scene = scene.borrow_mut();
            let at = position(&scene, id);
            if let Some(mut a) = scene.world.nav_agent_mut(id) {
                reset_path(&mut a, at);
            }
            Ok(())
        }),
    )
}
