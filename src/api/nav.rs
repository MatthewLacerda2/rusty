//! src/api/nav.rs — `Navigation` + `NavMeshAgent` namespaces.
//!
//! `Navigation.GetNextPathStep` queries the shared nav graph; `NavMeshAgent.*`
//! reads and writes an entity's optional nav-agent component. Scoped callbacks
//! over the borrowed scene + nav graph (#57).

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Scope};

use super::{put, Reg};
use crate::navigation::NavigationGraph;
use crate::scene::Scene;

/// Register the `Navigation` and `NavMeshAgent` namespaces onto `lua`.
pub fn register<'a, 'scope>(
    scope: &Scope<'a, 'scope>,
    lua: &'a Lua,
    scene: &'scope RefCell<&'scope mut Scene>,
    nav: &'scope RefCell<&'scope mut NavigationGraph>,
) -> Reg {
    register_navigation(scope, lua, nav)?;
    register_agent(scope, lua, scene)
}

/// `Navigation.GetNextPathStep` over the shared nav graph.
fn register_navigation<'a, 'scope>(
    scope: &Scope<'a, 'scope>,
    lua: &'a Lua,
    nav: &'scope RefCell<&'scope mut NavigationGraph>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "GetNextPathStep",
        scope.create_function(
            move |_, (cx, cy, cz, tx, ty, tz): (f32, f32, f32, f32, f32, f32)| {
                let step = nav
                    .borrow()
                    .get_next_path_step(Vec3::new(cx, cy, cz), Vec3::new(tx, ty, tz));
                Ok((step.x, step.y, step.z))
            },
        ),
    )?;

    lua.globals()
        .set("Navigation", table)
        .map_err(|e| e.to_string())
}

/// `NavMeshAgent.*` target / speed / radius accessors over the nav-agent component.
fn register_agent<'a, 'scope>(
    scope: &Scope<'a, 'scope>,
    lua: &'a Lua,
    scene: &'scope RefCell<&'scope mut Scene>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "SetTarget",
        scope.create_function(move |_, (id, x, y, z): (u32, f32, f32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(agent) = &mut e.nav_agent {
                    agent.target = Vec3::new(x, y, z);
                }
            }
            Ok(())
        }),
    )?;

    put(
        &table,
        "GetTarget",
        scope.create_function(move |_, id: u32| {
            let scene = scene.borrow();
            if let Some(e) = scene.get_entity(id) {
                if let Some(agent) = &e.nav_agent {
                    let t = agent.target;
                    return Ok((t.x, t.y, t.z));
                }
            }
            Ok((0.0, 0.0, 0.0))
        }),
    )?;

    put(
        &table,
        "SetSpeed",
        scope.create_function(move |_, (id, speed): (u32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(agent) = &mut e.nav_agent {
                    agent.speed = speed;
                }
            }
            Ok(())
        }),
    )?;

    put(
        &table,
        "SetAcceleration",
        scope.create_function(move |_, (id, acc): (u32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(agent) = &mut e.nav_agent {
                    agent.acceleration = acc;
                }
            }
            Ok(())
        }),
    )?;

    put(
        &table,
        "SetStoppingDistance",
        scope.create_function(move |_, (id, dist): (u32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(agent) = &mut e.nav_agent {
                    agent.stopping_distance = dist;
                }
            }
            Ok(())
        }),
    )?;

    put(
        &table,
        "SetRadius",
        scope.create_function(move |_, (id, r): (u32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(agent) = &mut e.nav_agent {
                    agent.radius = r;
                }
            }
            Ok(())
        }),
    )?;

    put(
        &table,
        "IsAtTarget",
        scope.create_function(move |_, id: u32| {
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
        &table,
        "GetVelocity",
        scope.create_function(move |_, id: u32| {
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
        &table,
        "SetActive",
        scope.create_function(move |_, (id, active): (u32, bool)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(agent) = &mut e.nav_agent {
                    agent.active = active;
                }
            }
            Ok(())
        }),
    )?;

    lua.globals()
        .set("NavMeshAgent", table)
        .map_err(|e| e.to_string())
}
