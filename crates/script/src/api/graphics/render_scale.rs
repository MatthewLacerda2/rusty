//! crates/script/src/api/graphics/render_scale.rs — the scene's render scale (#1498, #1715).
//!
//! `Get/SetRenderScale` and `ResetRenderScale` are the player's choice in the shared
//! graphics cell, which the shell persists to `Storage` (`graphics.render_scale`) at
//! Stop and quit; with no choice the project's default runs, so `ResetRenderScale`
//! is a game's "reset to default". `Get/SetDefaultRenderScale` are that default, on
//! the `Application` resource (`project.rusty`'s `"graphics"` block): a factor, or
//! `"match_display"` for `1 / Video.GetDisplayScale()`. In the editor the setter
//! writes `project.rusty`, as the Build Settings window does; in the player and the
//! harness it changes the value in memory only. Render-only.

use super::super::{put, Reg};
use crate::api::ApiCell;
use rusty_core::application::Application;
use rusty_core::settings::render_scale::{RenderScale, RenderScaleDefault, MATCH_DISPLAY};
use rusty_core::settings::GraphicsSettings;
use rusty_ui::ScreenSize;

/// Register the render-scale verbs.
pub(super) fn register(
    lua: &mlua::Lua,
    table: &mlua::Table,
    (graphics, screen): (ApiCell<GraphicsSettings>, ApiCell<ScreenSize>),
    app: ApiCell<Application>,
) -> Reg {
    put(
        table,
        "GetRenderScale",
        lua.create_function(move |lua, ()| {
            let display = screen.get(lua)?.borrow().display_scale();
            Ok(graphics
                .get(lua)?
                .borrow()
                .render_scale
                .resolve(display)
                .get())
        }),
    )?;
    put(
        table,
        "SetRenderScale",
        lua.create_function(move |lua, scale: f32| {
            graphics.get(lua)?.borrow_mut().render_scale.chosen = Some(RenderScale::new(scale));
            Ok(())
        }),
    )?;
    put(
        table,
        "ResetRenderScale",
        lua.create_function(move |lua, ()| {
            graphics.get(lua)?.borrow_mut().render_scale.chosen = None;
            Ok(())
        }),
    )?;
    register_default(lua, table, graphics, app)
}

/// The project default's `GetDefaultRenderScale` / `SetDefaultRenderScale`.
fn register_default(
    lua: &mlua::Lua,
    table: &mlua::Table,
    graphics: ApiCell<GraphicsSettings>,
    app: ApiCell<Application>,
) -> Reg {
    put(
        table,
        "GetDefaultRenderScale",
        lua.create_function(move |lua, ()| {
            Ok(match app.get(lua)?.borrow().render_scale_default() {
                RenderScaleDefault::Fixed(scale) => mlua::Value::Number(scale.get().into()),
                RenderScaleDefault::MatchDisplay => {
                    mlua::Value::String(lua.create_string(MATCH_DISPLAY)?)
                }
            })
        }),
    )?;
    put(
        table,
        "SetDefaultRenderScale",
        lua.create_function(move |lua, value: mlua::Value| {
            let refused =
                |why| mlua::Error::RuntimeError(format!("Graphics.SetDefaultRenderScale: {why}"));
            let json = crate::api::lua_json::lua_to_json(&value).map_err(refused)?;
            let default = RenderScaleDefault::read_json(&json).map_err(refused)?;
            graphics.with(lua, |graphics| {
                let live = &mut graphics.borrow_mut().render_scale;
                (app.get(lua)?.borrow_mut())
                    .set_default_render_scale(default, live)
                    .map_err(mlua::Error::RuntimeError)
            })
        }),
    )
}
