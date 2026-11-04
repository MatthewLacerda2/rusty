//! src/api/physics/volume.rs — `Physics.Overlap*` / `Physics.Check*`.
//!
//! Area queries over the live rapier world (#311): which entities' colliders
//! are inside a sphere/box/capsule *right now* (Unity `Physics.OverlapSphere` /
//! `OverlapBox` / `OverlapCapsule`), plus the boolean `CheckSphere` /
//! `CheckBox` fast-path for "is anything there?". Overlap results are arrays of
//! entity ids sorted ascending. Every verb takes an optional trailing
//! `query_triggers` after `layer_mask` (#1720; absent hits triggers); with no live physics world (edit mode) every
//! overlap is empty and every check is false.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Table;

use super::super::{put, Reg};
use super::cast::Filter;
use crate::api::ApiCell;
use crate::physics::PhysicsWorld;
use crate::scene::Scene;

/// Register the overlap/check family onto the `Physics` table.
pub(super) fn register(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    physics: ApiCell<Option<PhysicsWorld>>,
) -> Reg {
    register_sphere(lua, table, scene, physics)?;
    register_box(lua, table, scene, physics)?;
    register_capsule(lua, table, scene, physics)
}

/// Ids overlapping the sphere at `center`, mask-filtered; empty with no world.
fn sphere_ids(
    physics: &RefCell<Option<PhysicsWorld>>,
    scene: &RefCell<Scene>,
    center: Vec3,
    radius: f32,
    filter: Filter,
) -> Vec<u32> {
    let physics = physics.borrow();
    let Some(world) = physics.as_ref() else {
        return Vec::new();
    };
    let scene = scene.borrow();
    world.overlap_sphere(center, radius, |id| filter.accepts(&scene, id))
}

/// Ids overlapping the axis-aligned box at `center`, mask-filtered.
fn box_ids(
    physics: &RefCell<Option<PhysicsWorld>>,
    scene: &RefCell<Scene>,
    center: Vec3,
    half_extents: Vec3,
    filter: Filter,
) -> Vec<u32> {
    let physics = physics.borrow();
    let Some(world) = physics.as_ref() else {
        return Vec::new();
    };
    let scene = scene.borrow();
    world.overlap_box(center, half_extents, |id| filter.accepts(&scene, id))
}

/// `OverlapSphere` (id array) and `CheckSphere` (boolean fast-path).
fn register_sphere(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    physics: ApiCell<Option<PhysicsWorld>>,
) -> Reg {
    type Args = (f32, f32, f32, f32, Option<u32>, Option<bool>);
    put(
        table,
        "OverlapSphere",
        lua.create_function(move |lua, (cx, cy, cz, radius, mask, triggers): Args| {
            let ids = sphere_ids(
                &*physics.get(lua)?,
                &*scene.get(lua)?,
                Vec3::new(cx, cy, cz),
                radius,
                Filter::new(None, mask, triggers),
            );
            lua.create_sequence_from(ids)
        }),
    )?;
    put(
        table,
        "CheckSphere",
        lua.create_function(move |lua, (cx, cy, cz, radius, mask, triggers): Args| {
            Ok(!sphere_ids(
                &*physics.get(lua)?,
                &*scene.get(lua)?,
                Vec3::new(cx, cy, cz),
                radius,
                Filter::new(None, mask, triggers),
            )
            .is_empty())
        }),
    )
}

/// `OverlapBox` (id array) and `CheckBox` (boolean fast-path). Half-extents,
/// like Unity's `halfExtents`; the box is axis-aligned.
fn register_box(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    physics: ApiCell<Option<PhysicsWorld>>,
) -> Reg {
    type Args = (f32, f32, f32, f32, f32, f32, Option<u32>, Option<bool>);
    put(
        table,
        "OverlapBox",
        lua.create_function(move |lua, (cx, cy, cz, hx, hy, hz, mask, triggers): Args| {
            let ids = box_ids(
                &*physics.get(lua)?,
                &*scene.get(lua)?,
                Vec3::new(cx, cy, cz),
                Vec3::new(hx, hy, hz),
                Filter::new(None, mask, triggers),
            );
            lua.create_sequence_from(ids)
        }),
    )?;
    put(
        table,
        "CheckBox",
        lua.create_function(move |lua, (cx, cy, cz, hx, hy, hz, mask, triggers): Args| {
            let ids = box_ids(
                &*physics.get(lua)?,
                &*scene.get(lua)?,
                Vec3::new(cx, cy, cz),
                Vec3::new(hx, hy, hz),
                Filter::new(None, mask, triggers),
            );
            Ok(!ids.is_empty())
        }),
    )
}

/// `OverlapCapsule` — the capsule spans the two sphere centers `p0`→`p1` with
/// `radius`, mirroring Unity's point0/point1/radius form.
fn register_capsule(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    physics: ApiCell<Option<PhysicsWorld>>,
) -> Reg {
    type Args = (f32, f32, f32, f32, f32, f32, f32, Option<u32>, Option<bool>);
    put(
        table,
        "OverlapCapsule",
        lua.create_function(move |lua, args: Args| {
            let (x0, y0, z0, x1, y1, z1, radius, mask, triggers) = args;
            let filter = Filter::new(None, mask, triggers);
            let scene = &*scene.get(lua)?;
            let physics = &*physics.get(lua)?;
            let ids = {
                let physics = physics.borrow();
                match physics.as_ref() {
                    Some(world) => {
                        let scene = scene.borrow();
                        world.overlap_capsule(
                            Vec3::new(x0, y0, z0),
                            Vec3::new(x1, y1, z1),
                            radius,
                            |id| filter.accepts(&scene, id),
                        )
                    }
                    None => Vec::new(),
                }
            };
            lua.create_sequence_from(ids)
        }),
    )
}
