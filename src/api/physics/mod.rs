//! src/api/physics/mod.rs — `Physics` namespace.
//!
//! Rigidbody linear/angular velocity, force, and kinematic controls plus the
//! spatial query surface
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
    register_angular_velocity(scope, &table, scene)?;
    register_force(scope, &table, scene)?;
    register_collision_detection(scope, &table, scene)?;

    lua.globals()
        .set("Physics", table)
        .map_err(|e| e.to_string())
}

/// `"Discrete"`/`"Continuous"` (case-insensitive) <-> [`CollisionDetection`],
/// mirroring the string-enum bridge `Graphics.SetQuality` uses.
fn parse_collision_detection(name: &str) -> Option<CollisionDetection> {
    match name.to_ascii_lowercase().as_str() {
        "discrete" => Some(CollisionDetection::Discrete),
        "continuous" => Some(CollisionDetection::Continuous),
        _ => None,
    }
}

fn collision_detection_name(mode: CollisionDetection) -> &'static str {
    match mode {
        CollisionDetection::Discrete => "Discrete",
        CollisionDetection::Continuous => "Continuous",
    }
}

/// `GetCollisionDetection` / `SetCollisionDetection` — the Discrete/Continuous
/// (CCD) mode over the entity's rigidbody (#321). An unrecognized mode string is
/// a no-op, same as an unrecognized `Graphics.SetQuality` name.
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
                .get_entity(id)
                .and_then(|e| e.rigidbody.as_ref().map(|rb| rb.collision_detection))
                .unwrap_or_default();
            Ok(collision_detection_name(mode).to_string())
        }),
    )?;

    put(
        table,
        "SetCollisionDetection",
        scope.create_function(|_, (id, mode): (u32, String)| {
            let Some(mode) = parse_collision_detection(&mode) else {
                return Ok(());
            };
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                rb_ops::set_collision_detection(&mut e, mode);
            }
            Ok(())
        }),
    )
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

/// `GetAngularVelocity` / `SetAngularVelocity` (radians/sec per axis, Unity
/// `Rigidbody.angularVelocity`) over the entity's rigidbody. `SetAngularVelocity`
/// is a no-op on a kinematic or static body (#319) — see
/// `rb_ops::set_angular_velocity`.
fn register_angular_velocity<'lua, 'scope>(
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
                    let v = rb.angular_velocity;
                    return Ok((v.x, v.y, v.z));
                }
            }
            Ok((0.0, 0.0, 0.0))
        }),
    )?;

    put(
        table,
        "SetAngularVelocity",
        scope.create_function(|_, (id, vx, vy, vz): (u32, f32, f32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                rb_ops::set_angular_velocity(&mut e, Vec3::new(vx, vy, vz));
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
