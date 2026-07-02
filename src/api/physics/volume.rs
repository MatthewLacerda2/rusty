//! src/api/physics/volume.rs — `Physics.Overlap*` / `Physics.Check*`.
//!
//! Area queries over the live rapier world (#311): which entities' colliders
//! are inside a sphere/box/capsule *right now* (Unity `Physics.OverlapSphere` /
//! `OverlapBox` / `OverlapCapsule`), plus the boolean `CheckSphere` /
//! `CheckBox` fast-path for "is anything there?". Overlap results are arrays of
//! entity ids sorted ascending; with no live physics world (edit mode) every
//! overlap is empty and every check is false.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Table;

use super::super::{put, Reg};
use super::cast::accepts;
use crate::physics::PhysicsWorld;
use crate::scene::Scene;

/// Register the overlap/check family onto the `Physics` table.
pub(super) fn register<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
    physics: &'scope RefCell<Option<PhysicsWorld>>,
) -> Reg {
    register_sphere(scope, table, scene, physics)?;
    register_box(scope, table, scene, physics)?;
    register_capsule(scope, table, scene, physics)
}

/// Ids overlapping the sphere at `center`, mask-filtered; empty with no world.
fn sphere_ids(
    physics: &RefCell<Option<PhysicsWorld>>,
    scene: &RefCell<Scene>,
    center: Vec3,
    radius: f32,
    mask: Option<u32>,
) -> Vec<u32> {
    let physics = physics.borrow();
    let Some(world) = physics.as_ref() else {
        return Vec::new();
    };
    let scene = scene.borrow();
    world.overlap_sphere(center, radius, |id| accepts(&scene, id, None, mask))
}

/// Ids overlapping the axis-aligned box at `center`, mask-filtered.
fn box_ids(
    physics: &RefCell<Option<PhysicsWorld>>,
    scene: &RefCell<Scene>,
    center: Vec3,
    half_extents: Vec3,
    mask: Option<u32>,
) -> Vec<u32> {
    let physics = physics.borrow();
    let Some(world) = physics.as_ref() else {
        return Vec::new();
    };
    let scene = scene.borrow();
    world.overlap_box(center, half_extents, |id| accepts(&scene, id, None, mask))
}

/// `OverlapSphere` (id array) and `CheckSphere` (boolean fast-path).
fn register_sphere<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
    physics: &'scope RefCell<Option<PhysicsWorld>>,
) -> Reg {
    type Args = (f32, f32, f32, f32, Option<u32>);
    put(
        table,
        "OverlapSphere",
        scope.create_function(|lua, (cx, cy, cz, radius, mask): Args| {
            let ids = sphere_ids(physics, scene, Vec3::new(cx, cy, cz), radius, mask);
            lua.create_sequence_from(ids)
        }),
    )?;
    put(
        table,
        "CheckSphere",
        scope.create_function(|_, (cx, cy, cz, radius, mask): Args| {
            Ok(!sphere_ids(physics, scene, Vec3::new(cx, cy, cz), radius, mask).is_empty())
        }),
    )
}

/// `OverlapBox` (id array) and `CheckBox` (boolean fast-path). Half-extents,
/// like Unity's `halfExtents`; the box is axis-aligned.
fn register_box<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
    physics: &'scope RefCell<Option<PhysicsWorld>>,
) -> Reg {
    type Args = (f32, f32, f32, f32, f32, f32, Option<u32>);
    put(
        table,
        "OverlapBox",
        scope.create_function(|lua, (cx, cy, cz, hx, hy, hz, mask): Args| {
            let ids = box_ids(
                physics,
                scene,
                Vec3::new(cx, cy, cz),
                Vec3::new(hx, hy, hz),
                mask,
            );
            lua.create_sequence_from(ids)
        }),
    )?;
    put(
        table,
        "CheckBox",
        scope.create_function(|_, (cx, cy, cz, hx, hy, hz, mask): Args| {
            let ids = box_ids(
                physics,
                scene,
                Vec3::new(cx, cy, cz),
                Vec3::new(hx, hy, hz),
                mask,
            );
            Ok(!ids.is_empty())
        }),
    )
}

/// `OverlapCapsule` — the capsule spans the two sphere centers `p0`→`p1` with
/// `radius`, mirroring Unity's point0/point1/radius form.
fn register_capsule<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &Table,
    scene: &'scope RefCell<Scene>,
    physics: &'scope RefCell<Option<PhysicsWorld>>,
) -> Reg {
    type Args = (f32, f32, f32, f32, f32, f32, f32, Option<u32>);
    put(
        table,
        "OverlapCapsule",
        scope.create_function(|lua, (x0, y0, z0, x1, y1, z1, radius, mask): Args| {
            let ids = {
                let physics = physics.borrow();
                match physics.as_ref() {
                    Some(world) => {
                        let scene = scene.borrow();
                        world.overlap_capsule(
                            Vec3::new(x0, y0, z0),
                            Vec3::new(x1, y1, z1),
                            radius,
                            |id| accepts(&scene, id, None, mask),
                        )
                    }
                    None => Vec::new(),
                }
            };
            lua.create_sequence_from(ids)
        }),
    )
}
