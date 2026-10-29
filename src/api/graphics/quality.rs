//! src/api/graphics/quality.rs — the project's quality presets (#1522).
//!
//! `GetQuality` / `SetQuality` select a preset by name (refused, with the list,
//! when the project has none of that name); `QualityPresets` lists them with their
//! values; `SetQualityPreset` / `RemoveQualityPreset` author them. The presets live
//! on the `Application` resource, which writes `project.rusty` in the editor (in the
//! player and the harness they change in memory only), and every write puts the
//! running preset's values into the shared graphics cell the renderer reads next
//! frame. The player's choice persists through `Storage` at Stop and quit.
//! Render-only.

use std::cell::RefCell;

use mlua::LuaSerdeExt;

use super::super::{put, Reg};
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
pub(super) fn register_quality<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    graphics: &'scope RefCell<GraphicsSettings>,
    app: &'scope RefCell<Application>,
) -> Reg {
    put(
        table,
        "GetQuality",
        scope.create_function(|_, ()| Ok(app.borrow().quality().map(str::to_string))),
    )?;
    put(
        table,
        "SetQuality",
        scope.create_function(|_, name: String| {
            let mut app = app.borrow_mut();
            app.select_quality(&name)
                .map_err(|e| refused("SetQuality", e))?;
            sync(graphics, &app);
            Ok(())
        }),
    )?;
    put(
        table,
        "QualityPresets",
        scope.create_function(|lua, ()| lua.to_value(&app.borrow().quality_presets().presets)),
    )?;
    register_authoring(scope, table, graphics, app)
}

/// `SetQualityPreset(name, values)` (a partial edit over the preset of that name, or
/// a new one over the engine's values) and `RemoveQualityPreset(name)`.
fn register_authoring<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    graphics: &'scope RefCell<GraphicsSettings>,
    app: &'scope RefCell<Application>,
) -> Reg {
    put(
        table,
        "SetQualityPreset",
        scope.create_function(|lua, (name, values): (String, mlua::Value)| {
            let patch: serde_json::Value = lua.from_value(values)?;
            let mut app = app.borrow_mut();
            let preset = app.quality_presets().edited(&name, &patch);
            let preset = preset.map_err(|e| refused("SetQualityPreset", e))?;
            app.set_quality_preset(preset)
                .map_err(|e| refused("SetQualityPreset", e))?;
            sync(graphics, &app);
            Ok(())
        }),
    )?;
    put(
        table,
        "RemoveQualityPreset",
        scope.create_function(|_, name: String| {
            let mut app = app.borrow_mut();
            app.remove_quality_preset(&name)
                .map_err(|e| refused("RemoveQualityPreset", e))?;
            sync(graphics, &app);
            Ok(())
        }),
    )
}
