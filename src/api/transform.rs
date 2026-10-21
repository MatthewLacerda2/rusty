//! src/api/transform.rs — `Transform` namespace.
//!
//! Get/Set position, rotation (euler), scale, and `MoveTowards`. A write moves the
//! entity's subtree, colliders included (`ecs::world_space`, #1157).

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;

use super::{put, Reg};
use crate::scene::Scene;

/// Register the `Transform` namespace onto `lua`.
pub fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_position(scope, &table, scene)?;
    register_rotation(scope, &table, scene)?;
    register_scale(scope, &table, scene)?;
    register_move_towards(scope, &table, scene)?;

    lua.globals()
        .set("Transform", table)
        .map_err(|e| e.to_string())
}

/// `GetPosition` / `SetPosition`.
fn register_position<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GetPosition",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            let pos = scene.world.transform(id).map(|t| t.position);
            match pos {
                Some(pos) => Ok((pos.x, pos.y, pos.z)),
                None => Ok((0.0, 0.0, 0.0)),
            }
        }),
    )?;

    put(
        table,
        "SetPosition",
        scope.create_function(|_, (id, x, y, z): (u32, f32, f32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut t) = scene.world.transform_mut(id) {
                t.position = Vec3::new(x, y, z);
            }
            Ok(())
        }),
    )
}

/// `GetRotation` / `SetRotation` over euler angles.
fn register_rotation<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GetRotation",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            let rot = scene.world.transform(id).map(|t| t.euler_angles());
            match rot {
                Some(rot) => Ok((rot.x, rot.y, rot.z)),
                None => Ok((0.0, 0.0, 0.0)),
            }
        }),
    )?;

    put(
        table,
        "SetRotation",
        scope.create_function(|_, (id, x, y, z): (u32, f32, f32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut t) = scene.world.transform_mut(id) {
                t.set_euler_angles(Vec3::new(x, y, z));
            }
            Ok(())
        }),
    )
}

/// `GetScale` / `SetScale`.
fn register_scale<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GetScale",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            let scl = scene.world.transform(id).map(|t| t.scale);
            match scl {
                Some(scl) => Ok((scl.x, scl.y, scl.z)),
                None => Ok((1.0, 1.0, 1.0)),
            }
        }),
    )?;

    put(
        table,
        "SetScale",
        scope.create_function(|_, (id, x, y, z): (u32, f32, f32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut t) = scene.world.transform_mut(id) {
                t.scale = Vec3::new(x, y, z);
            }
            Ok(())
        }),
    )
}

/// `MoveTowards` — step toward a target, snapping when within `step`: glam's
/// `move_towards` (Unity's `Vector3.MoveTowards`), which a negative `step` walks away
/// from the target (#997).
fn register_move_towards<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "MoveTowards",
        scope.create_function(|_, (id, tx, ty, tz, step): (u32, f32, f32, f32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut t) = scene.world.transform_mut(id) {
                t.position = t.position.move_towards(Vec3::new(tx, ty, tz), step);
            }
            Ok(())
        }),
    )
}
