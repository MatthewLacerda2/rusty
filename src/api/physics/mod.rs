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
use crate::components::CollisionDetection;
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
    register_angular(scope, &table, scene)?;
    register_force(scope, &table, scene)?;
    register_collision_detection(scope, &table, scene)?;

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
            if let Some(mut c) = scene.world.rigidbody_mut(id) {
                rb_ops::set_velocity(&mut c, Vec3::new(vx, vy, vz));
            }
            Ok(())
        }),
    )
}

/// `GetAngularVelocity` / `SetAngularVelocity` (radians/sec per axis, Unity
/// `Rigidbody.angularVelocity`) over the entity's rigidbody. The setter no-ops on
/// kinematic/static bodies at the physics layer, mirroring `AddForce` and Unity.
fn register_angular<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GetAngularVelocity",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            if let Some(e) = scene.get_entity(id) {
                if let Some(rb) = &e.rigidbody {
                    let w = rb.angular_velocity;
                    return Ok((w.x, w.y, w.z));
                }
            }
            Ok((0.0, 0.0, 0.0))
        }),
    )?;

    put(
        table,
        "SetAngularVelocity",
        scope.create_function(|_, (id, wx, wy, wz): (u32, f32, f32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.rigidbody_mut(id) {
                rb_ops::set_angular_velocity(&mut c, Vec3::new(wx, wy, wz));
            }
            Ok(())
        }),
    )
}

/// `GetCollisionDetection` / `SetCollisionDetection` (Unity
/// `Rigidbody.collisionDetectionMode`, the two-mode subset). The mode is the
/// string `"Discrete"` or `"Continuous"`; an unknown string is a script error.
fn register_collision_detection<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GetCollisionDetection",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            let mode = scene
                .world.rigidbody(id).map(|rb| rb.collision_detection)
                .unwrap_or_default();
            Ok(mode.as_str().to_string())
        }),
    )?;

    put(
        table,
        "SetCollisionDetection",
        scope.create_function(|_, (id, mode): (u32, String)| {
            let mode = CollisionDetection::parse(&mode).ok_or_else(|| {
                mlua::Error::RuntimeError(format!(
                    "SetCollisionDetection: unknown mode {mode:?} (want \"Discrete\" or \"Continuous\")"
                ))
            })?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.rigidbody_mut(id) {
                rb_ops::set_collision_detection(&mut c, mode);
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
            if let Some(mut c) = scene.world.rigidbody_mut(id) {
                rb_ops::set_kinematic(&mut c, is_kinematic);
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
