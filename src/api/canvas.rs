//! src/api/canvas.rs — `Canvas` namespace (#417).
//!
//! Get/Set over an entity's `CanvasComponent` — render mode, sort order and the
//! *Scale With Screen Size* scaler (reference resolution, width/height match) —
//! plus the scale factor those produce on the current screen. Every setter routes
//! through the shared `scene::authoring::canvas` ops the inspector card uses.
//! Getters return a neutral default when the entity has no canvas.

use std::cell::RefCell;

use glam::Vec2;
use mlua::Lua;

use super::{put, Reg};
use crate::core::video::VideoSettings;
use crate::scene::authoring::canvas as canvas_ops;
use crate::scene::Scene;
use crate::ui::ScreenSize;

/// Register the `Canvas` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
    screen: &'scope RefCell<ScreenSize>,
    video: &'scope RefCell<VideoSettings>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    register_order_and_mode(scope, &table, scene)?;
    register_scaler(scope, &table, scene)?;
    put(
        &table,
        "GetScaleFactor",
        scope.create_function(|_, id: u32| {
            let px = screen.borrow().pixels(&video.borrow());
            let scene = scene.borrow();
            Ok(scene.world.canvas(id).map(|c| c.scale_factor(px)))
        }),
    )?;
    lua.globals()
        .set("Canvas", table)
        .map_err(|e| e.to_string())
}

/// `GetSortOrder` / `SetSortOrder` and `GetRenderMode` / `SetRenderMode` (by name;
/// an unknown name is ignored).
fn register_order_and_mode<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GetSortOrder",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            Ok(scene.world.canvas(id).map(|c| c.sort_order).unwrap_or(0))
        }),
    )?;
    put(
        table,
        "SetSortOrder",
        scope.create_function(|_, (id, order): (u32, i32)| {
            if let Some(mut c) = scene.borrow_mut().world.canvas_mut(id) {
                canvas_ops::set_sort_order(&mut c, order);
            }
            Ok(())
        }),
    )?;
    put(
        table,
        "GetRenderMode",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            let mode = scene.world.canvas(id).map(|c| c.render_mode);
            Ok(mode
                .map_or("None", canvas_ops::render_mode_name)
                .to_string())
        }),
    )?;
    put(
        table,
        "SetRenderMode",
        scope.create_function(|_, (id, name): (u32, String)| {
            let mode = canvas_ops::parse_render_mode(&name);
            if let (Some(mut c), Some(mode)) = (scene.borrow_mut().world.canvas_mut(id), mode) {
                canvas_ops::set_render_mode(&mut c, mode);
            }
            Ok(())
        }),
    )
}

/// The scaler: `Get/SetReferenceResolution` (each axis ≥ 1) and
/// `Get/SetMatchWidthOrHeight` (clamped to `[0, 1]`).
fn register_scaler<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GetReferenceResolution",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            let r = scene.world.canvas(id).map(|c| c.reference_resolution);
            let r = r.unwrap_or(Vec2::ZERO);
            Ok((r.x, r.y))
        }),
    )?;
    put(
        table,
        "SetReferenceResolution",
        scope.create_function(|_, (id, w, h): (u32, f32, f32)| {
            if let Some(mut c) = scene.borrow_mut().world.canvas_mut(id) {
                canvas_ops::set_reference_resolution(&mut c, Vec2::new(w, h));
            }
            Ok(())
        }),
    )?;
    put(
        table,
        "GetMatchWidthOrHeight",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            let m = scene.world.canvas(id).map(|c| c.match_width_or_height);
            Ok(m.unwrap_or(0.0))
        }),
    )?;
    put(
        table,
        "SetMatchWidthOrHeight",
        scope.create_function(|_, (id, value): (u32, f32)| {
            if let Some(mut c) = scene.borrow_mut().world.canvas_mut(id) {
                canvas_ops::set_match_width_or_height(&mut c, value);
            }
            Ok(())
        }),
    )
}
