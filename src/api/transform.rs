//! src/api/transform.rs — `Transform` namespace.
//!
//! Get/Set position, rotation (euler), scale, and `MoveTowards`. Mutations route
//! through `Scene::update_entity_collider` so a moved entity's collider tracks it.
//!
//! Registered as scoped callbacks over a borrowed `&RefCell<&mut Scene>` (#57):
//! each call re-borrows the same owned scene the systems hold, for the run's scope.

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Scope};

use super::{put, Reg};
use crate::scene::Scene;

/// Register the `Transform` namespace onto `lua`.
pub fn register<'a, 'scope>(
    scope: &Scope<'a, 'scope>,
    lua: &'a Lua,
    scene: &'scope RefCell<&'scope mut Scene>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "GetPosition",
        scope.create_function(move |_, id: u32| {
            let scene = scene.borrow();
            let pos = scene.get_entity(id).map(|e| e.transform.position);
            match pos {
                Some(pos) => Ok((pos.x, pos.y, pos.z)),
                None => Ok((0.0, 0.0, 0.0)),
            }
        }),
    )?;

    put(
        &table,
        "SetPosition",
        scope.create_function(move |_, (id, x, y, z): (u32, f32, f32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                e.transform.position = Vec3::new(x, y, z);
            }
            scene.update_entity_collider(id);
            Ok(())
        }),
    )?;

    put(
        &table,
        "GetRotation",
        scope.create_function(move |_, id: u32| {
            let scene = scene.borrow();
            let rot = scene.get_entity(id).map(|e| e.transform.euler_angles());
            match rot {
                Some(rot) => Ok((rot.x, rot.y, rot.z)),
                None => Ok((0.0, 0.0, 0.0)),
            }
        }),
    )?;

    put(
        &table,
        "SetRotation",
        scope.create_function(move |_, (id, x, y, z): (u32, f32, f32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                e.transform.set_euler_angles(Vec3::new(x, y, z));
            }
            scene.update_entity_collider(id);
            Ok(())
        }),
    )?;

    put(
        &table,
        "GetScale",
        scope.create_function(move |_, id: u32| {
            let scene = scene.borrow();
            let scl = scene.get_entity(id).map(|e| e.transform.scale);
            match scl {
                Some(scl) => Ok((scl.x, scl.y, scl.z)),
                None => Ok((1.0, 1.0, 1.0)),
            }
        }),
    )?;

    put(
        &table,
        "SetScale",
        scope.create_function(move |_, (id, x, y, z): (u32, f32, f32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                e.transform.scale = Vec3::new(x, y, z);
            }
            scene.update_entity_collider(id);
            Ok(())
        }),
    )?;

    put(
        &table,
        "MoveTowards",
        scope.create_function(
            move |_, (id, tx, ty, tz, step): (u32, f32, f32, f32, f32)| {
                let mut scene = scene.borrow_mut();
                if let Some(mut e) = scene.get_entity_mut(id) {
                    let pos = e.transform.position;
                    let target = Vec3::new(tx, ty, tz);
                    let dir = target - pos;
                    let len = dir.length();
                    if len <= step || len < 0.001 {
                        e.transform.position = target;
                    } else {
                        e.transform.position += dir.normalize() * step;
                    }
                }
                scene.update_entity_collider(id);
                Ok(())
            },
        ),
    )?;

    lua.globals()
        .set("Transform", table)
        .map_err(|e| e.to_string())
}
