//! src/api/transform.rs — `Transform` namespace.
//!
//! Unity's naming (#1170): `GetPosition` / `SetPosition` and the rotation pair are
//! **world** space, read and written through `ecs::world_space`; the `GetLocal*` /
//! `SetLocal*` pairs are the entity's own Transform, relative to its parent. Scale is
//! Unity's split too: `SetLocalScale` is the one scale a script writes, and
//! `GetLossyScale` reads the world one back. `MoveTowards` steps the world position.
//! A write moves the entity's subtree, colliders included (#1157).

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;

use super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::components::transform::{euler_degrees, from_euler_degrees};
use crate::ecs::World;
use crate::scene::Scene;

/// A read of `id` as a vector; `None` for a missing entity.
type Read = fn(&World, u32) -> Option<Vec3>;
/// A write of a vector to `id`; a no-op for a missing entity.
type Write = fn(&mut World, u32, Vec3);
/// A map from one space to another for a vector `id` gives.
type Map = fn(&World, u32, Vec3) -> Option<Vec3>;

/// Register the `Transform` namespace onto `lua`.
pub fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    for (name, read, default) in READS {
        getter(scope, &table, scene, name, *read, *default)?;
    }
    for (name, write) in WRITES {
        setter(scope, &table, scene, name, *write)?;
    }
    for (name, map) in MAPS {
        mapper(scope, &table, scene, name, *map)?;
    }
    register_move_towards(scope, &table, scene)?;
    lua.globals()
        .set("Transform", table)
        .map_err(|e| e.to_string())
}

const ZERO: Vec3 = Vec3::ZERO;

/// The getters, each with what a missing entity reads as (the world ones read
/// the API's own dead-id defaults, which are the same values).
const READS: &[(&str, Read, Vec3)] = &[
    ("GetPosition", |w, id| Some(w.world_position(id)), ZERO),
    (
        "GetLocalPosition",
        |w, id| Some(w.transform(id)?.position),
        ZERO,
    ),
    (
        "GetRotation",
        |w, id| Some(euler_degrees(w.world_rotation(id))),
        ZERO,
    ),
    (
        "GetLocalRotation",
        |w, id| Some(w.transform(id)?.euler_angles()),
        ZERO,
    ),
    (
        "GetLocalScale",
        |w, id| Some(w.transform(id)?.scale),
        Vec3::ONE,
    ),
    ("GetLossyScale", |w, id| Some(w.lossy_scale(id)), Vec3::ONE),
];

const WRITES: &[(&str, Write)] = &[
    ("SetPosition", |w, id, v| {
        w.set_world_position(id, v);
    }),
    ("SetLocalPosition", |w, id, v| {
        if let Some(mut t) = w.transform_mut(id) {
            t.position = v;
        }
    }),
    ("SetRotation", |w, id, v| {
        w.set_world_rotation(id, from_euler_degrees(v));
    }),
    ("SetLocalRotation", |w, id, v| {
        if let Some(mut t) = w.transform_mut(id) {
            t.set_euler_angles(v);
        }
    }),
    ("SetLocalScale", |w, id, v| {
        if let Some(mut t) = w.transform_mut(id) {
            t.scale = v;
        }
    }),
];

/// The space maps, `(id, x, y, z) -> x, y, z`. A missing entity maps the vector as
/// the identity; `InverseTransformPoint` gives `nil` under a singular world matrix.
const MAPS: &[(&str, Map)] = &[
    ("TransformPoint", |w, id, p| Some(w.transform_point(id, p))),
    ("TransformDirection", |w, id, d| {
        Some(w.transform_direction(id, d))
    }),
    ("InverseTransformPoint", |w, id, p| {
        w.inverse_transform_point(id, p)
    }),
];

fn getter<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    name: &str,
    read: Read,
    default: Vec3,
) -> Reg {
    let f = scope.create_function(move |_, id: EntityArg| {
        let v = read(&scene.borrow().world, id.read()).unwrap_or(default);
        Ok((v.x, v.y, v.z))
    });
    put(table, name, f)
}

fn setter<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    name: &str,
    write: Write,
) -> Reg {
    let verb = name.to_string();
    let f = scope.create_function(move |_, (id, x, y, z): (EntityArg, f32, f32, f32)| {
        let id = id.live("Transform", &verb)?;
        write(&mut scene.borrow_mut().world, id, Vec3::new(x, y, z));
        Ok(())
    });
    put(table, name, f)
}

fn mapper<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    name: &str,
    map: Map,
) -> Reg {
    let f = scope.create_function(move |_, (id, x, y, z): (EntityArg, f32, f32, f32)| {
        let v = map(&scene.borrow().world, id.read(), Vec3::new(x, y, z));
        Ok(v.map_or((None, None, None), |v| (Some(v.x), Some(v.y), Some(v.z))))
    });
    put(table, name, f)
}

/// `MoveTowards` — step the world position toward a world target, snapping when
/// within `step`: glam's `move_towards` (Unity's `Vector3.MoveTowards`), which a
/// negative `step` walks away from the target (#997).
fn register_move_towards<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let f = scope.create_function(
        |_, (id, tx, ty, tz, step): (EntityArg, f32, f32, f32, f32)| {
            let id = id.live("Transform", "MoveTowards")?;
            let world = &mut scene.borrow_mut().world;
            // A dead id reads the origin and the write is a no-op.
            let at = world
                .world_position(id)
                .move_towards(Vec3::new(tx, ty, tz), step);
            world.set_world_position(id, at);
            Ok(())
        },
    );
    put(table, "MoveTowards", f)
}
