//! src/api/nav/agent.rs — the `NavMeshAgent` namespace.
//!
//! Reads and writes an entity's optional nav-agent component: destination, motion
//! tuning, footprint, and the runtime queries the steering tick feeds.

use glam::Vec3;
use mlua::Lua;

use super::super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::navigation::{is_at_target, NavigationGraph};
use crate::scene::authoring::nav::agent as nav_ops;
use crate::scene::Scene;

/// `NavMeshAgent.*` target / speed / radius accessors over the nav-agent component, and
/// its path state (`path_state`, #458).
pub fn register(lua: &Lua, scene: ApiCell<Scene>, nav: ApiCell<NavigationGraph>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_agent_target(lua, &table, scene)?;
    register_agent_motion(lua, &table, scene)?;
    register_agent_size(lua, &table, scene)?;
    register_agent_queries(lua, &table, scene)?;
    register_agent_avoidance(lua, &table, scene)?;
    super::turning::register(lua, &table, scene)?;
    super::path_state::register_reads(lua, &table, scene)?;
    super::path_state::register_writes(lua, &table, scene, nav)?;
    super::offmesh::register_agent(lua, &table, scene)?;
    super::areas::register_agent(lua, &table, scene)?;

    lua.globals()
        .set("NavMeshAgent", table)
        .map_err(|e| e.to_string())
}

/// `SetTarget` / `GetTarget` over the agent's destination.
fn register_agent_target(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetTarget",
        lua.create_function(move |lua, (id, x, y, z): (EntityArg, f32, f32, f32)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("NavMeshAgent", "SetTarget")?;
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
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let t = scene
                .world
                .nav_agent(id.read())
                .map(|a| a.target)
                .unwrap_or(Vec3::ZERO);
            Ok((t.x, t.y, t.z))
        }),
    )
}

/// Motion tuning: `SetSpeed` / `SetAcceleration`.
fn register_agent_motion(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetSpeed",
        lua.create_function(move |lua, (id, speed): (EntityArg, f32)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("NavMeshAgent", "SetSpeed")?;
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
        lua.create_function(move |lua, (id, acc): (EntityArg, f32)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("NavMeshAgent", "SetAcceleration")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.nav_agent_mut(id) {
                nav_ops::set_acceleration(&mut c, acc);
            }
            Ok(())
        }),
    )
}

/// Footprint tuning: `SetStoppingDistance` / `SetRadius`, then [`register_base_offset`].
fn register_agent_size(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetStoppingDistance",
        lua.create_function(move |lua, (id, dist): (EntityArg, f32)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("NavMeshAgent", "SetStoppingDistance")?;
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
        lua.create_function(move |lua, (id, r): (EntityArg, f32)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("NavMeshAgent", "SetRadius")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.nav_agent_mut(id) {
                nav_ops::set_radius(&mut c, r);
            }
            Ok(())
        }),
    )?;
    register_base_offset(lua, table, scene)
}

/// `SetBaseOffset` / `GetBaseOffset`: the agent's height above the navmesh.
fn register_base_offset(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetBaseOffset",
        lua.create_function(move |lua, (id, offset): (EntityArg, f32)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("NavMeshAgent", "SetBaseOffset")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.nav_agent_mut(id) {
                nav_ops::set_base_offset(&mut c, offset);
            }
            Ok(())
        }),
    )?;
    put(
        table,
        "GetBaseOffset",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            Ok(scene
                .world
                .nav_agent(id.read())
                .map_or(0.0, |a| a.base_offset))
        }),
    )
}

/// Runtime queries / toggle: `IsAtTarget`, `GetVelocity`, and `SetActive`.
fn register_agent_queries(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "IsAtTarget",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let id = id.read();
            let scene = scene.borrow();
            let at_target = scene
                .world
                .nav_agent(id)
                .map(|agent| is_at_target(&agent, scene.world.world_position(id)))
                .unwrap_or(true);
            Ok(at_target)
        }),
    )?;

    put(
        table,
        "GetVelocity",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let v = scene
                .world
                .nav_agent(id.read())
                .map(|a| a.velocity)
                .unwrap_or(Vec3::ZERO);
            Ok((v.x, v.y, v.z))
        }),
    )?;

    put(
        table,
        "SetActive",
        lua.create_function(move |lua, (id, active): (EntityArg, bool)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("NavMeshAgent", "SetActive")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.nav_agent_mut(id) {
                nav_ops::set_active(&mut c, active);
            }
            Ok(())
        }),
    )
}

/// Local avoidance (#463): `SetAvoidancePriority` / `SetAvoidanceEnabled`.
fn register_agent_avoidance(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetAvoidancePriority",
        lua.create_function(move |lua, (id, priority): (EntityArg, f64)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("NavMeshAgent", "SetAvoidancePriority")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.nav_agent_mut(id) {
                nav_ops::set_avoidance_priority(&mut c, priority);
            }
            Ok(())
        }),
    )?;

    put(
        table,
        "SetAvoidanceEnabled",
        lua.create_function(move |lua, (id, enabled): (EntityArg, bool)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("NavMeshAgent", "SetAvoidanceEnabled")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.nav_agent_mut(id) {
                nav_ops::set_avoidance_enabled(&mut c, enabled);
            }
            Ok(())
        }),
    )
}
