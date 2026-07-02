//! src/api/physics/mod.rs — `Physics` namespace.
//!
//! Rigidbody velocity/force/kinematic controls plus the spatial query surface
//! over the live rapier/parry `PhysicsWorld` — the same query pipeline the
//! engine uses, so script and engine queries agree (#31, #311). The queries
//! split by shape: line casts (`Raycast`/`SphereCast`) in `cast`, volume
//! overlaps (`Overlap*`/`Check*`) in `volume`, per-collider point queries
//! (`ClosestPoint`/`ContainsPoint`/`GetBounds`) in `point`.
//! `register` creates the `Physics` table; `register_hitscan` extends it once
//! the live physics handle is available.

mod cast;
mod point;
mod volume;

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;

use super::{global_table, put, Reg};
use crate::physics::PhysicsWorld;
use crate::scene::authoring::rigidbody as rb_ops;
use crate::scene::Scene;
use crate::time::FIXED_DELTA_TIME;

/// Register the rigidbody half of `Physics` (velocity/force/kinematic) onto
/// `lua`, creating the `Physics` global table.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_velocity(scope, &table, scene)?;
    register_force(scope, &table, scene)?;

    lua.globals()
        .set("Physics", table)
        .map_err(|e| e.to_string())
}

/// `GetVelocity` / `SetVelocity` over the entity's rigidbody.
fn register_velocity<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GetVelocity",
        scope.create_function(|_, id: u32| {
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
        table,
        "SetVelocity",
        scope.create_function(|_, (id, vx, vy, vz): (u32, f32, f32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                rb_ops::set_velocity(&mut e, Vec3::new(vx, vy, vz));
            }
            Ok(())
        }),
    )
}

/// `AddForce` (continuous force, Unity `ForceMode.Force`) / `SetKinematic` over the
/// entity's rigidbody.
fn register_force<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "AddForce",
        scope.create_function(|_, (id, fx, fy, fz): (u32, f32, f32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(rb) = &mut e.rigidbody {
                    if !rb.is_kinematic {
                        // Continuous force (Unity `ForceMode.Force`): a force F over a
                        // fixed step dt changes velocity by F/m · dt. Scaling by the
                        // fixed step makes one call's effect match the physics tick it
                        // feeds, instead of an unscaled (dt-independent) velocity jump.
                        let dv = Vec3::new(fx, fy, fz) / rb.mass.max(0.0001) * FIXED_DELTA_TIME;
                        rb.velocity += dv;
                    }
                }
            }
            Ok(())
        }),
    )?;

    put(
        table,
        "SetKinematic",
        scope.create_function(|_, (id, is_kinematic): (u32, bool)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                rb_ops::set_kinematic(&mut e, is_kinematic);
            }
            Ok(())
        }),
    )
}

/// Extend `Physics` with the spatial query surface (#31, #311): the line casts
/// (`Raycast`/`SphereCast`), the volume overlaps (`OverlapSphere`/`OverlapBox`/
/// `OverlapCapsule` + `CheckSphere`/`CheckBox`), and the per-collider point
/// queries (`ClosestPoint`/`ContainsPoint`/`GetBounds`). All route through the
/// live rapier/parry `PhysicsWorld` — the same query pipeline the engine uses —
/// so a script's query and the engine's return identical answers.
pub fn register_hitscan<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
    physics: &'scope RefCell<Option<PhysicsWorld>>,
) -> Reg {
    let table = global_table(lua, "Physics")?;

    cast::register(scope, &table, scene, physics)?;
    volume::register(scope, &table, scene, physics)?;
    point::register(scope, &table, scene, physics)?;

    Ok(())
}
