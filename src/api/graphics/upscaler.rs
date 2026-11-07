//! src/api/graphics/upscaler.rs — the temporal upscaler a TAA camera runs (#1848).
//!
//! `Get/SetUpscaler` and `ResetUpscaler` are the player's choice in the shared
//! graphics cell, persisted to `Storage` (`graphics.upscaler`) at Stop and quit, as
//! the render scale's is; with no choice the project's default runs.
//! `Get/SetDefaultUpscaler` are that default, on the `Application` resource
//! (`project.rusty`'s `"graphics"` block). A name other than `"taa"` or `"fsr2"`
//! raises an error. Render-only.

use super::super::{put, Reg};
use crate::api::ApiCell;
use crate::core::application::Application;
use crate::core::settings::upscaler::Upscaler;
use crate::core::settings::GraphicsSettings;

/// `name` as an upscaler, or an error naming `verb`.
fn parse(verb: &str, name: &str) -> mlua::Result<Upscaler> {
    Upscaler::parse(name)
        .map_err(|why| mlua::Error::RuntimeError(format!("Graphics.{verb}: {why}")))
}

/// Register the upscaler verbs.
pub(super) fn register(
    lua: &mlua::Lua,
    table: &mlua::Table,
    graphics: ApiCell<GraphicsSettings>,
    app: ApiCell<Application>,
) -> Reg {
    put(
        table,
        "GetUpscaler",
        lua.create_function(move |lua, ()| {
            Ok(graphics.get(lua)?.borrow().upscaler.resolve().name())
        }),
    )?;
    put(
        table,
        "SetUpscaler",
        lua.create_function(move |lua, name: String| {
            let upscaler = parse("SetUpscaler", &name)?;
            graphics.get(lua)?.borrow_mut().upscaler.chosen = Some(upscaler);
            Ok(())
        }),
    )?;
    put(
        table,
        "ResetUpscaler",
        lua.create_function(move |lua, ()| {
            graphics.get(lua)?.borrow_mut().upscaler.chosen = None;
            Ok(())
        }),
    )?;
    put(
        table,
        "GetDefaultUpscaler",
        lua.create_function(move |lua, ()| Ok(app.get(lua)?.borrow().upscaler_default().name())),
    )?;
    put(
        table,
        "SetDefaultUpscaler",
        lua.create_function(move |lua, name: String| {
            let default = parse("SetDefaultUpscaler", &name)?;
            graphics.with(lua, |graphics| {
                let live = &mut graphics.borrow_mut().upscaler;
                (app.get(lua)?.borrow_mut())
                    .set_default_upscaler(default, live)
                    .map_err(mlua::Error::RuntimeError)
            })
        }),
    )
}
