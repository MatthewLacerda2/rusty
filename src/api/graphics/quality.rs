//! src/api/graphics/quality.rs — the project's quality presets (#1522).
//!
//! `GetQuality` / `SetQuality` select a preset by name (refused, with the list,
//! when the project has none of that name); `QualityPresets` lists them with their
//! values; `SetQualityPreset` / `RemoveQualityPreset` author them, and
//! `GetDefaultQuality` / `SetDefaultQuality` the one a first launch selects. The presets live
//! on the `Application` resource, which writes `project.rusty` in the editor (in the
//! player and the harness they change in memory only), and every write puts the
//! running preset's values into the shared graphics cell the renderer reads next
//! frame. The player's choice persists through `Storage` at Stop and quit.
//! Render-only.

use std::cell::RefCell;

use mlua::LuaSerdeExt;

use super::super::{put, Reg};
use crate::api::ApiCell;
use crate::core::application::Application;
use crate::core::settings::GraphicsSettings;

/// `Graphics.<function>`'s refusal of `why`.
fn refused(function: &str, why: impl std::fmt::Display) -> mlua::Error {
    mlua::Error::RuntimeError(format!("Graphics.{function}: {why}"))
}

/// Put the running preset's values into the cell after `app` changed.
fn sync(graphics: &RefCell<GraphicsSettings>, app: &Application) {
    graphics.borrow_mut().quality = app.quality_values();
}

/// Select, list and author the presets.
pub(super) fn register_quality(
    lua: &mlua::Lua,
    table: &mlua::Table,
    graphics: ApiCell<GraphicsSettings>,
    app: ApiCell<Application>,
) -> Reg {
    put(
        table,
        "GetQuality",
        lua.create_function(move |lua, ()| {
            Ok(app.get(lua)?.borrow().quality().map(str::to_string))
        }),
    )?;
    put(
        table,
        "SetQuality",
        lua.create_function(move |lua, name: String| {
            let app = &*app.get(lua)?;
            let mut app = app.borrow_mut();
            app.select_quality(&name)
                .map_err(|e| refused("SetQuality", e))?;
            sync(&*graphics.get(lua)?, &app);
            Ok(())
        }),
    )?;
    put(
        table,
        "QualityPresets",
        lua.create_function(move |lua, ()| {
            let app = &*app.get(lua)?;
            let mut app = app.borrow_mut();
            app.reread();
            lua.to_value(&app.quality_presets().presets)
        }),
    )?;
    put(
        table,
        "GetDefaultQuality",
        lua.create_function(move |lua, ()| Ok(app.get(lua)?.borrow_mut().default_quality())),
    )?;
    put(
        table,
        "SetDefaultQuality",
        lua.create_function(move |lua, name: Option<String>| {
            let app = &*app.get(lua)?;
            let mut app = app.borrow_mut();
            app.set_default_quality(name.as_deref())
                .map_err(|e| refused("SetDefaultQuality", e))?;
            sync(&*graphics.get(lua)?, &app);
            Ok(())
        }),
    )?;
    register_authoring(lua, table, graphics, app)
}

/// `SetQualityPreset(name, values)` (a partial edit over the preset of that name, or
/// a new one over the engine's values) and `RemoveQualityPreset(name)`.
fn register_authoring(
    lua: &mlua::Lua,
    table: &mlua::Table,
    graphics: ApiCell<GraphicsSettings>,
    app: ApiCell<Application>,
) -> Reg {
    put(
        table,
        "SetQualityPreset",
        lua.create_function(move |lua, (name, values): (String, mlua::Value)| {
            let app = &*app.get(lua)?;
            let patch: serde_json::Value = lua.from_value(values)?;
            let mut app = app.borrow_mut();
            app.edit_quality_preset(&name, &patch)
                .map_err(|e| refused("SetQualityPreset", e))?;
            sync(&*graphics.get(lua)?, &app);
            Ok(())
        }),
    )?;
    put(
        table,
        "RemoveQualityPreset",
        lua.create_function(move |lua, name: String| {
            let app = &*app.get(lua)?;
            let mut app = app.borrow_mut();
            app.remove_quality_preset(&name)
                .map_err(|e| refused("RemoveQualityPreset", e))?;
            sync(&*graphics.get(lua)?, &app);
            Ok(())
        }),
    )
}
