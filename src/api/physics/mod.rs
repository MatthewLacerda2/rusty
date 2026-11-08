//! src/api/physics/mod.rs — `Physics` namespace.
//!
//! Rigidbody velocity/force/kinematic controls plus the spatial query surface
//! over the live rapier/parry `PhysicsWorld` — the same query pipeline the
//! engine uses, so script and engine queries agree (#31, #311). The queries
//! split by shape: line and shape casts (`Raycast`, `SphereCast`/`CapsuleCast`/
//! `BoxCast`, the `*All` forms) in `cast`, volume overlaps (`Overlap*`/`Check*`)
//! in `volume`, per-collider point queries
//! (`ClosestPoint`/`ContainsPoint`/`GetBounds`) in `point`, and the collider's
//! authorable shape + inline physics material (#447) in `collider`, the shared
//! physics material assets (#1878) in `material`, the per-bone
//! hitbox generator (#464) in `hitbox`, forces / impulses at a point (#466)
//! in `impulse`, mass, gravity and the trigger flag (#1365) in `body`, the layer collision matrix (#827) in `matrix`, and the
//! enter/exit penetration cast (`RaycastThrough`, #830) in `through`.
//! `register` creates the `Physics` table; `register_hitscan` extends it once
//! the live physics handle is available.

mod body;
mod cast;
mod collider;
mod hitbox;
mod impulse;
mod material;
mod matrix;
mod point;
mod through;
mod volume;

use glam::Vec3;
use mlua::Lua;

use super::{global_table, put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::components::CollisionDetection;
use crate::physics::PhysicsWorld;
use crate::scene::authoring::physics::rigidbody as rb_ops;
use crate::scene::Scene;
use crate::time::FIXED_DELTA_TIME;

/// Register the rigidbody half of `Physics` (velocity/force/kinematic) onto
/// `lua`, creating the `Physics` global table.
pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_velocity(lua, &table, scene)?;
    register_angular(lua, &table, scene)?;
    register_force(lua, &table, scene)?;
    register_collision_detection(lua, &table, scene)?;
    body::register(lua, &table, scene)?;
    collider::register(lua, &table, scene)?;
    hitbox::register(lua, &table, scene)?;
    matrix::register(lua, &table, scene)?;
    material::register(lua, &table, scene)?;

    lua.globals()
        .set("Physics", table)
        .map_err(|e| e.to_string())
}

/// `GetVelocity` / `SetVelocity` over the entity's rigidbody.
fn register_velocity(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetVelocity",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let v = scene
                .world
                .rigidbody(id.read())
                .map(|rb| rb.velocity)
                .unwrap_or(Vec3::ZERO);
            Ok((v.x, v.y, v.z))
        }),
    )?;

    put(
        table,
        "SetVelocity",
        lua.create_function(move |lua, (id, vx, vy, vz): (EntityArg, f32, f32, f32)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Physics", "SetVelocity")?;
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
fn register_angular(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetAngularVelocity",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let w = scene
                .world
                .rigidbody(id.read())
                .map(|rb| rb.angular_velocity)
                .unwrap_or(Vec3::ZERO);
            Ok((w.x, w.y, w.z))
        }),
    )?;

    put(
        table,
        "SetAngularVelocity",
        lua.create_function(move |lua, (id, wx, wy, wz): (EntityArg, f32, f32, f32)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Physics", "SetAngularVelocity")?;
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
fn register_collision_detection(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
) -> Reg {
    put(
        table,
        "GetCollisionDetection",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let mode = scene
                .world
                .rigidbody(id.read())
                .map(|rb| rb.collision_detection)
                .unwrap_or_default();
            Ok(mode.name().to_string())
        }),
    )?;

    put(
        table,
        "SetCollisionDetection",
        lua.create_function(move |lua, (id, mode): (EntityArg, String)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Physics", "SetCollisionDetection")?;
            let mode = CollisionDetection::parse(&mode).ok_or_else(|| {
                mlua::Error::RuntimeError(format!(
                    "SetCollisionDetection: unknown mode {mode:?} (want one of {})",
                    CollisionDetection::names()
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
fn register_force(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "AddForce",
        lua.create_function(move |lua, (id, fx, fy, fz): (EntityArg, f32, f32, f32)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Physics", "AddForce")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut rb) = scene.world.rigidbody_mut(id) {
                if !rb.is_kinematic {
                    // Continuous force (Unity `ForceMode.Force`): a force F over a
                    // fixed step dt changes velocity by F/m · dt. Scaling by the
                    // fixed step makes one call's effect match the physics tick it
                    // feeds, instead of an unscaled (dt-independent) velocity jump.
                    let dv = Vec3::new(fx, fy, fz) / rb.mass.max(0.0001) * FIXED_DELTA_TIME;
                    rb.velocity += dv;
                }
            }
            Ok(())
        }),
    )?;

    put(
        table,
        "SetKinematic",
        lua.create_function(move |lua, (id, is_kinematic): (EntityArg, bool)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Physics", "SetKinematic")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.rigidbody_mut(id) {
                rb_ops::set_kinematic(&mut c, is_kinematic);
            }
            Ok(())
        }),
    )
}

/// Extend `Physics` with the spatial query surface (#31, #311, #1881): the
/// line and shape casts (`Raycast`, `SphereCast`/`CapsuleCast`/`BoxCast`,
/// `RaycastAll`/`SphereCastAll`), the volume overlaps (`Overlap*` +
/// `Check*` for spheres, oriented boxes and capsules), and the per-collider point
/// queries (`ClosestPoint`/`ContainsPoint`/`GetBounds`). All route through the
/// live rapier/parry `PhysicsWorld` — the same query pipeline the engine uses —
/// so a script's query and the engine's return identical answers.
pub fn register_hitscan(
    lua: &Lua,
    scene: ApiCell<Scene>,
    physics: ApiCell<Option<PhysicsWorld>>,
) -> Reg {
    let table = global_table(lua, "Physics")?;

    cast::register(lua, &table, scene, physics)?;
    through::register(lua, &table, scene, physics)?;
    volume::register(lua, &table, scene, physics)?;
    point::register(lua, &table, scene, physics)?;
    impulse::register(lua, &table, scene, physics)?;

    Ok(())
}
