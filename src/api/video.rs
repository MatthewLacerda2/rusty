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

use std::cell::RefCell;

use mlua::Lua;

use super::{put, Reg};
use crate::core::video::VideoSettings;

/// Register the `Video` namespace onto `lua`, backed by the shared settings cell.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    video: &'scope RefCell<VideoSettings>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_resolution(scope, &table, video)?;
    register_vsync(scope, &table, video)?;
    register_fullscreen(scope, &table, video)?;

    lua.globals().set("Video", table).map_err(|e| e.to_string())
}

/// Resolution get/set. `SetResolution(w, h)` clamps each axis to at least 1.
fn register_resolution<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    video: &'scope RefCell<VideoSettings>,
) -> Reg {
    put(
        table,
        "SetResolution",
        scope.create_function(|_, (w, h): (u32, u32)| {
            let mut s = video.borrow_mut();
            s.width = w.max(1);
            s.height = h.max(1);
            Ok(())
        }),
    )?;
    put(
        table,
        "GetResolution",
        scope.create_function(|_, ()| {
            let (w, h) = video.borrow().resolution();
            Ok((w, h))
        }),
    )
}

/// Vsync on/off get/set.
fn register_vsync<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    video: &'scope RefCell<VideoSettings>,
) -> Reg {
    put(
        table,
        "SetVsync",
        scope.create_function(|_, on: bool| {
            video.borrow_mut().vsync = on;
            Ok(())
        }),
    )?;
    put(
        table,
        "GetVsync",
        scope.create_function(|_, ()| Ok(video.borrow().vsync)),
    )
}

/// Fullscreen on/off get/set.
fn register_fullscreen<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    video: &'scope RefCell<VideoSettings>,
) -> Reg {
    put(
        table,
        "SetFullscreen",
        scope.create_function(|_, on: bool| {
            video.borrow_mut().fullscreen = on;
            Ok(())
        }),
    )?;
    put(
        table,
        "GetFullscreen",
        scope.create_function(|_, ()| Ok(video.borrow().fullscreen)),
    )
}
