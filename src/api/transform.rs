//! src/api/transform.rs — `Transform` namespace.
//!
//! Get/Set position, rotation (euler), scale, and `MoveTowards`. Mutations route
//! through `Scene::update_entity_collider` so a moved entity's collider tracks it.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use mlua::Lua;

use super::{put, Reg};
use crate::scene::Scene;

/// Register the `Transform` namespace onto `lua`.
#[allow(clippy::too_many_lines)]
pub fn register(lua: &Lua, scene: &Rc<RefCell<Scene>>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    let s = Rc::clone(scene);
    put(
        &table,
        "GetPosition",
        lua.create_function(move |_, id: u32| {
            let scene = s.borrow();
            let pos = scene.get_entity(id).map(|e| e.transform.position);
            match pos {
                Some(pos) => Ok((pos.x, pos.y, pos.z)),
                None => Ok((0.0, 0.0, 0.0)),
            }
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "SetPosition",
        lua.create_function(move |_, (id, x, y, z): (u32, f32, f32, f32)| {
            let mut scene = s.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                e.transform.position = Vec3::new(x, y, z);
            }
            scene.update_entity_collider(id);
            Ok(())
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "GetRotation",
        lua.create_function(move |_, id: u32| {
            let scene = s.borrow();
            let rot = scene.get_entity(id).map(|e| e.transform.euler_angles());
            match rot {
                Some(rot) => Ok((rot.x, rot.y, rot.z)),
                None => Ok((0.0, 0.0, 0.0)),
            }
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "SetRotation",
        lua.create_function(move |_, (id, x, y, z): (u32, f32, f32, f32)| {
            let mut scene = s.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                e.transform.set_euler_angles(Vec3::new(x, y, z));
            }
            scene.update_entity_collider(id);
            Ok(())
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "GetScale",
        lua.create_function(move |_, id: u32| {
            let scene = s.borrow();
            let scl = scene.get_entity(id).map(|e| e.transform.scale);
            match scl {
                Some(scl) => Ok((scl.x, scl.y, scl.z)),
                None => Ok((1.0, 1.0, 1.0)),
            }
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "SetScale",
        lua.create_function(move |_, (id, x, y, z): (u32, f32, f32, f32)| {
            let mut scene = s.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                e.transform.scale = Vec3::new(x, y, z);
            }
            scene.update_entity_collider(id);
            Ok(())
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "MoveTowards",
        lua.create_function(
            move |_, (id, tx, ty, tz, step): (u32, f32, f32, f32, f32)| {
                let mut scene = s.borrow_mut();
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
