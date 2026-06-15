//! src/api/camera.rs — `Camera` namespace.
//!
//! Get/Set for position, yaw, pitch and fov over the shared `render::Camera` the
//! simulation drives, plus `GetForward`/`GetRight` basis vectors so a controller
//! script can move and aim relative to where the camera looks. Scoped callbacks
//! over a borrowed `&RefCell<&mut Camera>` (#57).

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Lua, Scope};

use super::{put, Reg};
use crate::render::Camera;

/// Register the `Camera` namespace onto `lua`.
pub fn register<'a, 'scope>(
    scope: &Scope<'a, 'scope>,
    lua: &'a Lua,
    camera: &'scope RefCell<&'scope mut Camera>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "GetPosition",
        scope.create_function(move |_, ()| {
            let cam = camera.borrow();
            Ok((cam.position.x, cam.position.y, cam.position.z))
        }),
    )?;

    put(
        &table,
        "SetPosition",
        scope.create_function(move |_, (x, y, z): (f32, f32, f32)| {
            camera.borrow_mut().position = Vec3::new(x, y, z);
            Ok(())
        }),
    )?;

    // Forward / right basis vectors derived from yaw+pitch. Pure engine math
    // (`Camera::forward`/`right`) exposed so a controller script can move and aim
    // relative to where the camera looks, instead of re-deriving the trig in Lua.
    put(
        &table,
        "GetForward",
        scope.create_function(move |_, ()| {
            let f = camera.borrow().forward();
            Ok((f.x, f.y, f.z))
        }),
    )?;

    put(
        &table,
        "GetRight",
        scope.create_function(move |_, ()| {
            let r = camera.borrow().right();
            Ok((r.x, r.y, r.z))
        }),
    )?;

    put(
        &table,
        "GetYaw",
        scope.create_function(move |_, ()| Ok(camera.borrow().yaw)),
    )?;
    put(
        &table,
        "SetYaw",
        scope.create_function(move |_, yaw: f32| {
            camera.borrow_mut().yaw = yaw;
            Ok(())
        }),
    )?;

    put(
        &table,
        "GetPitch",
        scope.create_function(move |_, ()| Ok(camera.borrow().pitch)),
    )?;
    put(
        &table,
        "SetPitch",
        scope.create_function(move |_, pitch: f32| {
            camera.borrow_mut().pitch = pitch.clamp(-89.0, 89.0);
            Ok(())
        }),
    )?;

    put(
        &table,
        "GetFov",
        scope.create_function(move |_, ()| Ok(camera.borrow().fov)),
    )?;
    put(
        &table,
        "SetFov",
        scope.create_function(move |_, fov: f32| {
            camera.borrow_mut().fov = fov.clamp(1.0, 179.0);
            Ok(())
        }),
    )?;

    lua.globals()
        .set("Camera", table)
        .map_err(|e| e.to_string())
}
