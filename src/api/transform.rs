//! src/api/transform.rs — `Transform` namespace.
//!
//! Get/Set position, rotation (euler), scale, and `MoveTowards`. Mutations route
//! through `Scene::update_entity_collider` so a moved entity's collider tracks it.

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;

use super::{put, Reg};
use crate::scene::Scene;

/// Register the `Transform` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
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

/// `GetPosition` / `SetPosition` (set re-syncs the entity's collider).
fn register_position<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
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
            scene.update_entity_collider(id);
            Ok(())
        }),
    )
}

/// `GetRotation` / `SetRotation` over euler angles (set re-syncs the collider).
fn register_rotation<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
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
            scene.update_entity_collider(id);
            Ok(())
        }),
    )
}

/// `GetScale` / `SetScale` (set re-syncs the entity's collider).
fn register_scale<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
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
            scene.update_entity_collider(id);
            Ok(())
        }),
    )
}

/// `MoveTowards` — step toward a target, snapping when within `step` (or ~0).
fn register_move_towards<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "MoveTowards",
        scope.create_function(|_, (id, tx, ty, tz, step): (u32, f32, f32, f32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut t) = scene.world.transform_mut(id) {
                let pos = t.position;
                let target = Vec3::new(tx, ty, tz);
                let dir = target - pos;
                let len = dir.length();
                if len <= step || len < 0.001 {
                    t.position = target;
                } else {
                    t.position += dir.normalize() * step;
                }
            }
            scene.update_entity_collider(id);
            Ok(())
        }),
    )
}
