//! src/api/physics/point.rs — `Physics.ClosestPoint` / `ContainsPoint` /
//! `GetBounds`.
//!
//! Per-collider point queries (#311). `ClosestPoint`/`ContainsPoint` ask the
//! live rapier collider (play mode); `GetBounds` surfaces the world-space AABB
//! the engine already keeps current on the component
//! (`ColliderComponent::calculate_world_aabb`), so it answers in edit mode too.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Table;

use super::super::{put, Reg};
use crate::physics::PhysicsWorld;
use crate::scene::Scene;

/// Register the point-query family onto the `Physics` table.
pub(super) fn register<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
    physics: &'scope RefCell<Option<PhysicsWorld>>,
) -> Reg {
    register_closest(scope, table, physics)?;
    register_contains(scope, table, physics)?;
    register_bounds(scope, table, scene)
}

/// `ClosestPoint` — `(found, x, y, z)`: the nearest point on the entity's live
/// collider to the query point (Unity `Collider.ClosestPoint`; a point inside
/// is its own closest point). found=false ⇒ no live collider for that entity
/// (edit mode, or no collider) and the query point echoes back.
fn register_closest<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table,
    physics: &'scope RefCell<Option<PhysicsWorld>>,
) -> Reg {
    put(
        table,
        "ClosestPoint",
        scope.create_function(|_, (id, x, y, z): (u32, f32, f32, f32)| {
            let physics = physics.borrow();
            let closest = physics
                .as_ref()
                .and_then(|world| world.closest_point_on(id, Vec3::new(x, y, z)));
            Ok(match closest {
                Some(p) => (true, p.x, p.y, p.z),
                None => (false, x, y, z),
            })
        }),
    )
}

/// `ContainsPoint` — whether the point lies inside the entity's live collider;
/// `false` when the entity has no live collider (edit mode, or no collider).
fn register_contains<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table,
    physics: &'scope RefCell<Option<PhysicsWorld>>,
) -> Reg {
    put(
        table,
        "ContainsPoint",
        scope.create_function(|_, (id, x, y, z): (u32, f32, f32, f32)| {
            let physics = physics.borrow();
            Ok(physics
                .as_ref()
                .and_then(|world| world.collider_contains_point(id, Vec3::new(x, y, z)))
                .unwrap_or(false))
        }),
    )
}

/// `GetBounds` — `(found, min_x, min_y, min_z, max_x, max_y, max_z)`: the
/// collider's cached world-space AABB, recomputed by the scene on transform
/// edits, load, and each physics step. found=false ⇒ the entity has no
/// collider component and the six extents are 0.
fn register_bounds<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GetBounds",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            let bounds = scene
                .get_entity(id)
                .and_then(|e| e.collider.as_ref().map(|c| (c.aabb_min, c.aabb_max)));
            Ok(match bounds {
                Some((min, max)) => (true, min.x, min.y, min.z, max.x, max.y, max.z),
                None => (false, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            })
        }),
    )
}
