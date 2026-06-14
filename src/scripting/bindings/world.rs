//! src/scripting/bindings/world.rs — `Time`, `Camera`, writable `Input`, `Debug`.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use mlua::Lua;

use super::{global_table, put, Reg};
use crate::core::input::InputState;
use crate::render::Camera;
#[cfg(feature = "dev")]
use crate::scripting::ConsoleLogs;
use crate::time::Time;

/// `Time.deltaTime/fixedDeltaTime/frameCount` — read-only accessors.
pub fn register_time(lua: &Lua, time: &Rc<RefCell<Time>>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    let t = Rc::clone(time);
    put(
        &table,
        "deltaTime",
        lua.create_function(move |_, ()| Ok(t.borrow().delta_time)),
    )?;

    let t = Rc::clone(time);
    put(
        &table,
        "fixedDeltaTime",
        lua.create_function(move |_, ()| Ok(t.borrow().fixed_delta_time)),
    )?;

    let t = Rc::clone(time);
    put(
        &table,
        "frameCount",
        lua.create_function(move |_, ()| Ok(t.borrow().frame_count)),
    )?;

    lua.globals().set("Time", table).map_err(|e| e.to_string())
}

/// `Camera` get/set for position, yaw, pitch and fov.
pub fn register_camera(lua: &Lua, camera: &Rc<RefCell<Camera>>) -> Reg {
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

/// Add the writable half of `Input`: `Press`/`Release` drive the shared input
/// state so a script can play as the user (Unity-ish `Input` injection).
pub fn register_input_writable(lua: &Lua, input: &Rc<RefCell<InputState>>) -> Reg {
    let table = global_table(lua, "Input")?;

    let i = Rc::clone(input);
    put(
        &table,
        "Press",
        lua.create_function(move |_, key: String| {
            i.borrow_mut().press(&key);
            Ok(())
        }),
    )?;

    let i = Rc::clone(input);
    put(
        &table,
        "Release",
        lua.create_function(move |_, key: String| {
            i.borrow_mut().release(&key);
            Ok(())
        }),
    )?;

    Ok(())
}

/// `Debug.Log/Warn/Error` — DEV ONLY (stripped from ship builds).
#[cfg(feature = "dev")]
pub fn register_debug(lua: &Lua, console: &Rc<RefCell<ConsoleLogs>>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    let c = Rc::clone(console);
    put(
        &table,
        "Log",
        lua.create_function(move |_, msg: String| {
            c.borrow_mut().info(msg);
            Ok(())
        }),
    )?;

    let c = Rc::clone(console);
    put(
        &table,
        "Warn",
        lua.create_function(move |_, msg: String| {
            c.borrow_mut().warn(msg);
            Ok(())
        }),
    )?;

    let c = Rc::clone(console);
    put(
        &table,
        "Error",
        lua.create_function(move |_, msg: String| {
            c.borrow_mut().error(msg);
            Ok(())
        }),
    )?;

    lua.globals().set("Debug", table).map_err(|e| e.to_string())
}
