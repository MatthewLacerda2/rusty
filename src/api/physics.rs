//! src/api/physics.rs — `Physics` namespace.
//!
//! Rigidbody velocity/force/kinematic controls, plus `Raycast` / `Shoot`
//! hitscan cast against the live rapier/parry `PhysicsWorld` — the same query
//! pipeline the engine hitscan uses, so script and engine casts agree (#31).
//! `register` creates the `Physics` table; `register_hitscan` extends it once
//! the live physics handle is available.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use mlua::Lua;

use super::health::apply_damage;
use super::{global_table, put, Reg};
use crate::core::scene::Scene;
use crate::physics::{is_hittable, PhysicsWorld};
use crate::scripting::ConsoleLogs;

/// Register the rigidbody half of `Physics` (velocity/force/kinematic) onto
/// `lua`, creating the `Physics` global table.
pub fn register(lua: &Lua, scene: &Rc<RefCell<Scene>>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    let s = Rc::clone(scene);
    put(
        &table,
        "GetVelocity",
        lua.create_function(move |_, id: u32| {
            let scene = s.borrow();
            if let Some(e) = scene.get_entity(id) {
                if let Some(rb) = &e.rigidbody {
                    return Ok((rb.velocity.x, rb.velocity.y, rb.velocity.z));
                }
            }
            Ok((0.0, 0.0, 0.0))
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "SetVelocity",
        lua.create_function(move |_, (id, vx, vy, vz): (u32, f32, f32, f32)| {
            let mut scene = s.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(rb) = &mut e.rigidbody {
                    rb.velocity = Vec3::new(vx, vy, vz);
                }
            }
            Ok(())
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "AddForce",
        lua.create_function(move |_, (id, fx, fy, fz): (u32, f32, f32, f32)| {
            let mut scene = s.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(rb) = &mut e.rigidbody {
                    if !rb.is_kinematic {
                        let acc = Vec3::new(fx, fy, fz) / rb.mass.max(0.0001);
                        rb.velocity += acc;
                    }
                }
            }
            Ok(())
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "SetKinematic",
        lua.create_function(move |_, (id, is_kinematic): (u32, bool)| {
            let mut scene = s.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(rb) = &mut e.rigidbody {
                    rb.is_kinematic = is_kinematic;
                }
            }
            Ok(())
        }),
    )?;

    lua.globals()
        .set("Physics", table)
        .map_err(|e| e.to_string())
}

/// Extend `Physics` with `Raycast` (query) and `Shoot` (raycast + apply damage).
/// Both route through the live rapier/parry `PhysicsWorld` — the same query
/// pipeline the engine hitscan uses — so a script's cast and the engine's cast
/// return identical hits for the same ray. `Shoot` is the hitscan that used to be
/// inline in `app/play.rs`.
pub fn register_hitscan(
    lua: &Lua,
    scene: &Rc<RefCell<Scene>>,
    physics: &Rc<RefCell<Option<PhysicsWorld>>>,
    console: &Rc<RefCell<ConsoleLogs>>,
) -> Reg {
    let table = global_table(lua, "Physics")?;

    let s = Rc::clone(scene);
    let p = Rc::clone(physics);
    put(
        &table,
        "Raycast",
        lua.create_function(
            move |_, (ox, oy, oz, dx, dy, dz): (f32, f32, f32, f32, f32, f32)| {
                // (hit, entity_id, distance) — hit=false ⇒ id/dist are 0.
                match cast(&p, &s, Vec3::new(ox, oy, oz), Vec3::new(dx, dy, dz)) {
                    Some((id, t)) => Ok((true, id, t)),
                    None => Ok((false, 0u32, 0.0f32)),
                }
            },
        ),
    )?;

    let s = Rc::clone(scene);
    let p = Rc::clone(physics);
    let c = Rc::clone(console);
    put(
        &table,
        "Shoot",
        lua.create_function(
            move |_, (ox, oy, oz, dx, dy, dz, damage): (f32, f32, f32, f32, f32, f32, f32)| {
                match cast(&p, &s, Vec3::new(ox, oy, oz), Vec3::new(dx, dy, dz)) {
                    Some((id, t)) => {
                        apply_damage(&s, &c, id, damage);
                        Ok((true, id, t))
                    }
                    None => Ok((false, 0u32, 0.0f32)),
                }
            },
        ),
    )?;

    Ok(())
}

/// Cast `origin`→`dir` through the live rapier world under the shared hitscan
/// filter ([`is_hittable`]). Returns `None` when no physics world exists yet
/// (edit mode / no Play has built one) or on a miss.
fn cast(
    physics: &Rc<RefCell<Option<PhysicsWorld>>>,
    scene: &Rc<RefCell<Scene>>,
    origin: Vec3,
    dir: Vec3,
) -> Option<(u32, f32)> {
    let physics = physics.borrow();
    let physics = physics.as_ref()?;
    physics.cast_ray_filtered(origin, dir, f32::MAX, |id| is_hittable(&scene.borrow(), id))
}
