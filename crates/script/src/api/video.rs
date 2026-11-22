//! Writes land in the shared `VideoSettings` cell, which the platform layer applies to the
//! surface and window: no GPU is touched from script. Headless, the resolution is also the
//! UI's screen size (`ScreenSize::pixels`).
//! `GetDisplayScale` reads the display's backing scale off the `ScreenSize`
//! resource, a sim input like the Game view's size `UI.GetScreenSize` reads: 1
//! headless, and recorded by a demo, so a replay never depends on the monitor.

use mlua::Lua;

use super::{put, Reg};
use crate::api::ApiCell;
use rusty_core::settings::video::VideoSettings;
use rusty_ui::ScreenSize;

pub fn register(lua: &Lua, video: ApiCell<VideoSettings>, screen: ApiCell<ScreenSize>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "GetDisplayScale",
        lua.create_function(move |lua, ()| Ok(screen.get(lua)?.borrow().display_scale())),
    )?;

    register_resolution(lua, &table, video)?;
    register_vsync(lua, &table, video)?;
    register_fullscreen(lua, &table, video)?;

    lua.globals().set("Video", table).map_err(|e| e.to_string())
}

/// Resolution get/set. `SetResolution(w, h)` clamps each axis to at least 1.
fn register_resolution(lua: &mlua::Lua, table: &mlua::Table, video: ApiCell<VideoSettings>) -> Reg {
    put(
        table,
        "SetResolution",
        lua.create_function(move |lua, (w, h): (u32, u32)| {
            let video = &*video.get(lua)?;
            let mut s = video.borrow_mut();
            s.width = w.max(1);
            s.height = h.max(1);
            Ok(())
        }),
    )?;
    put(
        table,
        "GetResolution",
        lua.create_function(move |lua, ()| {
            let (w, h) = video.get(lua)?.borrow().resolution();
            Ok((w, h))
        }),
    )
}

fn register_vsync(lua: &mlua::Lua, table: &mlua::Table, video: ApiCell<VideoSettings>) -> Reg {
    put(
        table,
        "SetVsync",
        lua.create_function(move |lua, on: bool| {
            video.get(lua)?.borrow_mut().vsync = on;
            Ok(())
        }),
    )?;
    put(
        table,
        "GetVsync",
        lua.create_function(move |lua, ()| Ok(video.get(lua)?.borrow().vsync)),
    )
}

fn register_fullscreen(lua: &mlua::Lua, table: &mlua::Table, video: ApiCell<VideoSettings>) -> Reg {
    put(
        table,
        "SetFullscreen",
        lua.create_function(move |lua, on: bool| {
            video.get(lua)?.borrow_mut().fullscreen = on;
            Ok(())
        }),
    )?;
    put(
        table,
        "GetFullscreen",
        lua.create_function(move |lua, ()| Ok(video.get(lua)?.borrow().fullscreen)),
    )
}
