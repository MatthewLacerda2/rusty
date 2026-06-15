//! src/api/physics.rs — `Physics` namespace.
//!
//! Rigidbody velocity/force/kinematic controls, plus `Raycast` / `Shoot`
//! hitscan cast against the live rapier/parry `PhysicsWorld` — the same query
//! pipeline the engine hitscan uses, so script and engine casts agree (#31).
//! `register` creates the `Physics` table; `register_hitscan` extends it once
//! the live physics handle is available. Scoped callbacks over the borrowed
//! scene + physics + console cells (#57).

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Scope};

use super::health::apply_damage;
use super::{global_table, put, Reg};
use crate::physics::{is_hittable, PhysicsWorld};
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// Register the rigidbody half of `Physics` (velocity/force/kinematic) onto
/// `lua`, creating the `Physics` global table.
pub fn register<'a, 'scope>(
    scope: &Scope<'a, 'scope>,
    lua: &'a Lua,
    scene: &'scope RefCell<&'scope mut Scene>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "GetVelocity",
        scope.create_function(move |_, id: u32| {
            let scene = scene.borrow();
            if let Some(e) = scene.get_entity(id) {
                if let Some(rb) = &e.rigidbody {
                    return Ok((rb.velocity.x, rb.velocity.y, rb.velocity.z));
                }
            }
            Ok((0.0, 0.0, 0.0))
        }),
    )?;

    put(
        &table,
        "SetVelocity",
        scope.create_function(move |_, (id, vx, vy, vz): (u32, f32, f32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(rb) = &mut e.rigidbody {
                    rb.velocity = Vec3::new(vx, vy, vz);
                }
            }
            Ok(())
        }),
    )?;

    put(
        &table,
        "AddForce",
        scope.create_function(move |_, (id, fx, fy, fz): (u32, f32, f32, f32)| {
            let mut scene = scene.borrow_mut();
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

    put(
        &table,
        "SetKinematic",
        scope.create_function(move |_, (id, is_kinematic): (u32, bool)| {
            let mut scene = scene.borrow_mut();
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
pub fn register_hitscan<'a, 'scope>(
    scope: &Scope<'a, 'scope>,
    lua: &'a Lua,
    scene: &'scope RefCell<&'scope mut Scene>,
    physics: &'scope RefCell<&'scope mut Option<PhysicsWorld>>,
    console: &'scope RefCell<&'scope mut ConsoleLogs>,
) -> Reg {
    let table = global_table(lua, "Physics")?;

    put(
        &table,
        "Raycast",
        scope.create_function(
            move |_,
                  (ox, oy, oz, dx, dy, dz, ignore): (
                f32,
                f32,
                f32,
                f32,
                f32,
                f32,
                Option<u32>,
            )| {
                // (hit, entity_id, distance) — hit=false ⇒ id/dist are 0. The
                // optional trailing `ignore` id is skipped (e.g. the shooter).
                match cast(
                    physics,
                    scene,
                    Vec3::new(ox, oy, oz),
                    Vec3::new(dx, dy, dz),
                    ignore,
                ) {
                    Some((id, t)) => Ok((true, id, t)),
                    None => Ok((false, 0u32, 0.0f32)),
                }
            },
        ),
    )?;

    put(
        &table,
        "Shoot",
        scope.create_function(
            move |_,
                  (ox, oy, oz, dx, dy, dz, damage, ignore): (
                f32,
                f32,
                f32,
                f32,
                f32,
                f32,
                f32,
                Option<u32>,
            )| {
                match cast(
                    physics,
                    scene,
                    Vec3::new(ox, oy, oz),
                    Vec3::new(dx, dy, dz),
                    ignore,
                ) {
                    Some((id, t)) => {
                        apply_damage(scene, console, id, damage);
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
/// filter ([`is_hittable`]), additionally skipping `ignore` (the shooter's own
/// entity, so a shot can't hit its source). Returns `None` when no physics world
/// exists yet (edit mode / no Play has built one) or on a miss.
fn cast(
    physics: &RefCell<&mut Option<PhysicsWorld>>,
    scene: &RefCell<&mut Scene>,
    origin: Vec3,
    dir: Vec3,
    ignore: Option<u32>,
) -> Option<(u32, f32)> {
    let physics = physics.borrow();
    let physics = physics.as_ref()?;
    physics.cast_ray_filtered(origin, dir, f32::MAX, |id| {
        Some(id) != ignore && is_hittable(&scene.borrow(), id)
    })
}
