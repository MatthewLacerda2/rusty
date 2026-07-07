//! src/api/nav/agent.rs — the `NavMeshAgent` namespace.
//!
//! Reads and writes an entity's optional nav-agent component: destination, motion
//! tuning, footprint, and the runtime queries the steering tick feeds.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;

use super::super::{put, Reg};
use crate::scene::authoring::nav_agent as nav_ops;
use crate::scene::Scene;

/// `NavMeshAgent.*` target / speed / radius accessors over the nav-agent component.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_agent_target(scope, &table, scene)?;
    register_agent_motion(scope, &table, scene)?;
    register_agent_size(scope, &table, scene)?;
    register_agent_queries(scope, &table, scene)?;

    lua.globals()
        .set("NavMeshAgent", table)
        .map_err(|e| e.to_string())
}

/// `SetTarget` / `GetTarget` over the agent's destination.
fn register_agent_target<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetTarget",
        scope.create_function(|_, (id, x, y, z): (u32, f32, f32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.nav_agent_mut(id) {
                nav_ops::set_target(&mut c, Vec3::new(x, y, z));
            }
            Ok(())
        }),
    )?;

    put(
        table,
        "GetTarget",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            if let Some(e) = scene.get_entity(id) {
                if let Some(agent) = &e.nav_agent {
                    let t = agent.target;
                    return Ok((t.x, t.y, t.z));
                }
            }
            Ok((0.0, 0.0, 0.0))
        }),
    )
}

/// Motion tuning: `SetSpeed` / `SetAcceleration`.
fn register_agent_motion<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetSpeed",
        scope.create_function(|_, (id, speed): (u32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.nav_agent_mut(id) {
                nav_ops::set_speed(&mut c, speed);
            }
            Ok(())
        }),
    )?;

    put(
        table,
        "SetAcceleration",
        scope.create_function(|_, (id, acc): (u32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.nav_agent_mut(id) {
                nav_ops::set_acceleration(&mut c, acc);
            }
            Ok(())
        }),
    )
}

/// Footprint tuning: `SetStoppingDistance` / `SetRadius`.
fn register_agent_size<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetStoppingDistance",
        scope.create_function(|_, (id, dist): (u32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.nav_agent_mut(id) {
                nav_ops::set_stopping_distance(&mut c, dist);
            }
            Ok(())
        }),
    )?;

    put(
        table,
        "SetRadius",
        scope.create_function(|_, (id, r): (u32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.nav_agent_mut(id) {
                nav_ops::set_radius(&mut c, r);
            }
            Ok(())
        }),
    )
}

/// Runtime queries / toggle: `IsAtTarget`, `GetVelocity`, and `SetActive`.
fn register_agent_queries<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "IsAtTarget",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            if let Some(e) = scene.get_entity(id) {
                if let Some(agent) = &e.nav_agent {
                    let current_pos = e.transform.position;
                    let to_target = agent.target - current_pos;
                    return Ok(to_target.length() <= agent.stopping_distance);
                }
            }
            Ok(true)
        }),
    )?;

    put(
        table,
        "GetVelocity",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            if let Some(e) = scene.get_entity(id) {
                if let Some(agent) = &e.nav_agent {
                    let v = agent.velocity;
                    return Ok((v.x, v.y, v.z));
                }
            }
            Ok((0.0, 0.0, 0.0))
        }),
    )?;

    put(
        table,
        "SetActive",
        scope.create_function(|_, (id, active): (u32, bool)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.nav_agent_mut(id) {
                nav_ops::set_active(&mut c, active);
            }
            Ok(())
        }),
    )
}
