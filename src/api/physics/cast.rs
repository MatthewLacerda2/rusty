//! src/api/physics/cast.rs — `Physics.Raycast` / `Physics.SphereCast`.
//!
//! Query-only line casts through the live rapier world. Both share one
//! acceptance test (`accepts`) with the volume and point queries, so every
//! spatial query honours the same ignore / layer-mask rules (#91, #311).

use std::cell::RefCell;

use glam::Vec3;
use mlua::Table;

use super::super::{put, Reg};
use crate::physics::{is_hittable, PhysicsWorld};
use crate::scene::{layer_in_mask, Scene};

/// The one acceptance test every spatial query filters colliders through: skip
/// the optional `ignore` entity (e.g. the shooter), require the entity to be
/// hittable ([`is_hittable`]), and — when a layer mask (bit per layer) is given
/// — require the entity's layer bit to be set (#91). Rejected colliders are
/// passed through, never treated as blockers.
pub(super) fn accepts(scene: &Scene, id: u32, ignore: Option<u32>, mask: Option<u32>) -> bool {
    if Some(id) == ignore || !is_hittable(scene, id) {
        return false;
    }
    match mask {
        Some(m) => scene
            .get_entity(id)
            .is_some_and(|e| layer_in_mask(e.layer, m)),
        None => true,
    }
}

/// Fold an optional (id, distance) hit into the `(hit, entity_id, distance)`
/// triple both casts return — hit=false ⇒ id/dist are 0.
fn cast_result(hit: Option<(u32, f32)>) -> (bool, u32, f32) {
    match hit {
        Some((id, t)) => (true, id, t),
        None => (false, 0u32, 0.0f32),
    }
}

/// Register `Raycast` and `SphereCast` onto the `Physics` table.
pub(super) fn register<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
    physics: &'scope RefCell<Option<PhysicsWorld>>,
) -> Reg {
    register_raycast(scope, table, scene, physics)?;
    register_spherecast(scope, table, scene, physics)
}

/// `Raycast` — query-only cast returning `(hit, entity_id, distance)`.
fn register_raycast<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
    physics: &'scope RefCell<Option<PhysicsWorld>>,
) -> Reg {
    type Args = (f32, f32, f32, f32, f32, f32, Option<u32>, Option<u32>);
    put(
        table,
        "Raycast",
        scope.create_function(|_, (ox, oy, oz, dx, dy, dz, ignore, mask): Args| {
            // The optional trailing `ignore` id is skipped (e.g. the shooter);
            // the optional `mask` limits hits to entities on those layers (#91).
            // Returns `None` (⇒ miss) when no physics world exists yet (edit
            // mode / no Play has built one).
            let physics = physics.borrow();
            let scene = scene.borrow();
            Ok(cast_result(physics.as_ref().and_then(|world| {
                world.cast_ray_filtered(
                    Vec3::new(ox, oy, oz),
                    Vec3::new(dx, dy, dz),
                    f32::MAX,
                    |id| accepts(&scene, id, ignore, mask),
                )
            })))
        }),
    )
}

/// `SphereCast` — a raycast with a radius (Unity `Physics.SphereCast`): the
/// first accepted collider touched by a sphere swept along the ray. Same
/// `(hit, entity_id, distance)` contract as `Raycast`; distance is how far the
/// sphere's center traveled before impact.
fn register_spherecast<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
    physics: &'scope RefCell<Option<PhysicsWorld>>,
) -> Reg {
    type Args = (f32, f32, f32, f32, f32, f32, f32, Option<u32>, Option<u32>);
    put(
        table,
        "SphereCast",
        scope.create_function(|_, (ox, oy, oz, dx, dy, dz, radius, ignore, mask): Args| {
            let physics = physics.borrow();
            let scene = scene.borrow();
            Ok(cast_result(physics.as_ref().and_then(|world| {
                world.cast_sphere_filtered(
                    Vec3::new(ox, oy, oz),
                    Vec3::new(dx, dy, dz),
                    radius,
                    f32::MAX,
                    |id| accepts(&scene, id, ignore, mask),
                )
            })))
        }),
    )
}
