//! src/api/camera.rs — `Camera` namespace.
//!
//! Get/Set for position, yaw, pitch and fov over the shared `render::Camera` the
//! simulation drives, plus `GetForward`/`GetRight` basis vectors so a controller
//! script can move and aim relative to where the camera looks.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use mlua::Lua;

use super::{put, Reg};
use crate::render::Camera;

/// Register the `Camera` namespace onto `lua`.
pub fn register(lua: &Lua, camera: &Rc<RefCell<Camera>>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    let c = Rc::clone(camera);
    put(
        &table,
        "GetPosition",
        lua.create_function(move |_, ()| {
            let cam = c.borrow();
            Ok((cam.position.x, cam.position.y, cam.position.z))
        }),
    )?;

    let c = Rc::clone(camera);
    put(
        &table,
        "SetPosition",
        lua.create_function(move |_, (x, y, z): (f32, f32, f32)| {
            c.borrow_mut().position = Vec3::new(x, y, z);
            Ok(())
        }),
    )?;

    // Forward / right basis vectors derived from yaw+pitch. Pure engine math
    // (`Camera::forward`/`right`) exposed so a controller script can move and aim
    // relative to where the camera looks, instead of re-deriving the trig in Lua.
    let c = Rc::clone(camera);
    put(
        &table,
        "GetForward",
        lua.create_function(move |_, ()| {
            let f = c.borrow().forward();
            Ok((f.x, f.y, f.z))
        }),
    )?;

    let c = Rc::clone(camera);
    put(
        &table,
        "GetRight",
        lua.create_function(move |_, ()| {
            let r = c.borrow().right();
            Ok((r.x, r.y, r.z))
        }),
    )?;

    let c = Rc::clone(camera);
    put(
        &table,
        "GetYaw",
        lua.create_function(move |_, ()| Ok(c.borrow().yaw)),
    )?;
    let c = Rc::clone(camera);
    put(
        &table,
        "SetYaw",
        lua.create_function(move |_, yaw: f32| {
            c.borrow_mut().yaw = yaw;
            Ok(())
        }),
    )?;

    let c = Rc::clone(camera);
    put(
        &table,
        "GetPitch",
        lua.create_function(move |_, ()| Ok(c.borrow().pitch)),
    )?;
    let c = Rc::clone(camera);
    put(
        &table,
        "SetPitch",
        lua.create_function(move |_, pitch: f32| {
            c.borrow_mut().pitch = pitch.clamp(-89.0, 89.0);
            Ok(())
        }),
    )?;

    let c = Rc::clone(camera);
    put(
        &table,
        "GetFov",
        lua.create_function(move |_, ()| Ok(c.borrow().fov)),
    )?;
    let c = Rc::clone(camera);
    put(
        &table,
        "SetFov",
        lua.create_function(move |_, fov: f32| {
            c.borrow_mut().fov = fov.clamp(1.0, 179.0);
            Ok(())
        }),
    )?;

    lua.globals()
        .set("Camera", table)
        .map_err(|e| e.to_string())
}
