//! src/api/physics.rs — `Physics` namespace.
//!
//! Rigidbody velocity/force/kinematic controls, plus the `Raycast` hitscan cast
//! against the live rapier/parry `PhysicsWorld` — the same query pipeline the
//! engine hitscan uses, so script and engine casts agree (#31).
//! `register` creates the `Physics` table; `register_hitscan` extends it once
//! the live physics handle is available.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;

use super::{global_table, put, Reg};
use crate::physics::{is_hittable, PhysicsWorld};
use crate::scene::authoring::rigidbody as rb_ops;
use crate::scene::{layer_in_mask, Scene};
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

/// Extend `Physics` with `Raycast` (the query-only hitscan). It routes through the
/// live rapier/parry `PhysicsWorld` — the same query pipeline the engine hitscan
/// uses — so a script's cast and the engine's cast return identical hits for the
/// same ray.
pub fn register_hitscan<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
    physics: &'scope RefCell<Option<PhysicsWorld>>,
) -> Reg {
    let table = global_table(lua, "Physics")?;

    register_raycast(scope, &table, scene, physics)?;

    Ok(())
}

/// `Raycast` — query-only cast returning `(hit, entity_id, distance)`.
fn register_raycast<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    physics: &'scope RefCell<Option<PhysicsWorld>>,
) -> Reg {
    put(
        table,
        "Raycast",
        scope.create_function(
            |_,
             (ox, oy, oz, dx, dy, dz, ignore, mask): (
                f32,
                f32,
                f32,
                f32,
                f32,
                f32,
                Option<u32>,
                Option<u32>,
            )| {
                // (hit, entity_id, distance) — hit=false ⇒ id/dist are 0. The
                // optional trailing `ignore` id is skipped (e.g. the shooter); the
                // optional `mask` limits hits to entities on those layers (#91).
                match cast(
                    physics,
                    scene,
                    Vec3::new(ox, oy, oz),
                    Vec3::new(dx, dy, dz),
                    ignore,
                    mask,
                ) {
                    Some((id, t)) => Ok((true, id, t)),
                    None => Ok((false, 0u32, 0.0f32)),
                }
            },
        ),
    )
}

/// Cast `origin`→`dir` through the live rapier world under the shared hitscan
/// filter ([`is_hittable`]), additionally skipping `ignore` (the shooter's own
/// entity, so a shot can't hit its source). Returns `None` when no physics world
/// exists yet (edit mode / no Play has built one) or on a miss.
fn cast(
    physics: &RefCell<Option<PhysicsWorld>>,
    scene: &RefCell<Scene>,
    origin: Vec3,
    dir: Vec3,
    ignore: Option<u32>,
    mask: Option<u32>,
) -> Option<(u32, f32)> {
    let physics = physics.borrow();
    let physics = physics.as_ref()?;
    let scene = scene.borrow();
    physics.cast_ray_filtered(origin, dir, f32::MAX, |id| {
        if Some(id) == ignore || !is_hittable(&scene, id) {
            return false;
        }
        // A layer mask (bit per layer) limits hits to entities on those layers;
        // absent, every layer is eligible. Excluded colliders are passed through.
        match mask {
            Some(m) => scene
                .get_entity(id)
                .is_some_and(|e| layer_in_mask(e.layer, m)),
            None => true,
        }
    })
}
