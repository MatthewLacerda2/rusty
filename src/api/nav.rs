//! src/api/nav.rs — `Navigation` + `NavMeshAgent` namespaces.
//!
//! `Navigation.GetNextPathStep` queries the shared nav graph; `NavMeshAgent.*`
//! reads and writes an entity's optional nav-agent component.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use mlua::Lua;

use super::{put, Reg};
use crate::navigation::NavigationGraph;
use crate::scene::Scene;

/// Register the `Navigation` and `NavMeshAgent` namespaces onto `lua`.
pub fn register(lua: &Lua, scene: &Rc<RefCell<Scene>>, nav: &Rc<RefCell<NavigationGraph>>) -> Reg {
    register_navigation(lua, nav)?;
    register_agent(lua, scene)
}

/// `Navigation.GetNextPathStep` over the shared nav graph.
fn register_navigation(lua: &Lua, nav: &Rc<RefCell<NavigationGraph>>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    let n = Rc::clone(nav);
    put(
        &table,
        "GetNextPathStep",
        lua.create_function(
            move |_, (cx, cy, cz, tx, ty, tz): (f32, f32, f32, f32, f32, f32)| {
                let step = n
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
#[allow(clippy::too_many_lines)]
fn register_agent(lua: &Lua, scene: &Rc<RefCell<Scene>>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    let s = Rc::clone(scene);
    put(
        &table,
        "SetTarget",
        lua.create_function(move |_, (id, x, y, z): (u32, f32, f32, f32)| {
            let mut scene = s.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(agent) = &mut e.nav_agent {
                    agent.target = Vec3::new(x, y, z);
                }
            }
            Ok(())
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "GetTarget",
        lua.create_function(move |_, id: u32| {
            let scene = s.borrow();
            if let Some(e) = scene.get_entity(id) {
                if let Some(agent) = &e.nav_agent {
                    let t = agent.target;
                    return Ok((t.x, t.y, t.z));
                }
            }
            Ok((0.0, 0.0, 0.0))
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "SetSpeed",
        lua.create_function(move |_, (id, speed): (u32, f32)| {
            let mut scene = s.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(agent) = &mut e.nav_agent {
                    agent.speed = speed;
                }
            }
            Ok(())
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "SetAcceleration",
        lua.create_function(move |_, (id, acc): (u32, f32)| {
            let mut scene = s.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(agent) = &mut e.nav_agent {
                    agent.acceleration = acc;
                }
            }
            Ok(())
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "SetStoppingDistance",
        lua.create_function(move |_, (id, dist): (u32, f32)| {
            let mut scene = s.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(agent) = &mut e.nav_agent {
                    agent.stopping_distance = dist;
                }
            }
            Ok(())
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "SetRadius",
        lua.create_function(move |_, (id, r): (u32, f32)| {
            let mut scene = s.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(agent) = &mut e.nav_agent {
                    agent.radius = r;
                }
            }
            Ok(())
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "IsAtTarget",
        lua.create_function(move |_, id: u32| {
            let scene = s.borrow();
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

    let s = Rc::clone(scene);
    put(
        &table,
        "GetVelocity",
        lua.create_function(move |_, id: u32| {
            let scene = s.borrow();
            if let Some(e) = scene.get_entity(id) {
                if let Some(agent) = &e.nav_agent {
                    let v = agent.velocity;
                    return Ok((v.x, v.y, v.z));
                }
            }
            Ok((0.0, 0.0, 0.0))
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "SetActive",
        lua.create_function(move |_, (id, active): (u32, bool)| {
            let mut scene = s.borrow_mut();
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
