//! src/api/video.rs — `Video` namespace.
//!
//! Runtime control over the windowed app's **video** settings (issue #89):
//! framebuffer resolution, vsync, and fullscreen. Distinct from `Graphics`, which
//! drives per-volume post-FX knobs and the quality preset: video settings are
//! window/surface state, not scene-graph state, so they get their own namespace
//! and their own persistence (the `video` Storage namespace) — mirroring how Unity
//! separates `Screen`/`QualitySettings`.
//!
//! Writes land in a shared [`VideoSettings`] cell, the same one-way pattern as the
//! quality cell: the platform layer (`main.rs`) reads the cell each frame and, when
//! it differs from what the renderer/window currently show, reconfigures the wgpu
//! surface (resolution / present mode) and the winit window (fullscreen). No GPU is
//! touched from script. Every write is ONE-WAY into render-only state — it never
//! feeds `FixedUpdate`, so the deterministic sim is unaffected.

use mlua::Lua;

use super::{put, Reg};
use crate::api::ApiCell;
use crate::core::settings::video::VideoSettings;

/// Register the `Video` namespace onto `lua`, backed by the shared settings cell.
pub fn register(lua: &Lua, video: ApiCell<VideoSettings>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

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

/// Vsync on/off get/set.
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

/// Fullscreen on/off get/set.
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
