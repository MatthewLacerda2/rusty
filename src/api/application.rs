//! src/api/application.rs — `Application` namespace.
//!
//! Unity's `Application` class, the subset a shipped game needs (#431): quitting,
//! plus the project's build settings (startup scene, product name, window mode).
//!
//! `Quit` only **records** the request on the shared [`Application`] cell; the host
//! decides what it means — the standalone player closes (after flushing `Storage`),
//! the editor stops Play, the headless harness ends the run. Nothing here touches a
//! window, so the namespace is identical in every host and the sim stays window-free.
//!
//! The build-settings setters are the API twin of the editor's File → Build Settings
//! window (the parity rule). In the editor they write the tracked
//! `project/build_settings.json`; in the player and the harness the cell is unbound,
//! so a setter changes the in-memory value only.

use std::cell::RefCell;

use mlua::{Lua, Table};

use super::{put, Reg};
use crate::core::application::{Application, WindowMode};

/// Register the `Application` namespace onto `lua`, backed by the shared cell.
pub fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    app: &'scope RefCell<Application>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "Quit",
        scope.create_function(|_, ()| {
            app.borrow_mut().request_quit();
            Ok(())
        }),
    )?;
    register_names(scope, &table, app)?;
    register_window_mode(scope, &table, app)?;

    lua.globals()
        .set("Application", table)
        .map_err(|e| e.to_string())
}

/// Apply `edit` to the build settings, surfacing a failed file write as a Lua error.
fn edit(
    app: &RefCell<Application>,
    edit: impl FnOnce(&mut crate::core::application::BuildSettings),
) -> mlua::Result<()> {
    app.borrow_mut()
        .update(edit)
        .map_err(mlua::Error::RuntimeError)
}

/// `Get/SetProductName` and `Get/SetStartupScene`.
fn register_names<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &Table,
    app: &'scope RefCell<Application>,
) -> Reg {
    put(
        table,
        "GetProductName",
        scope.create_function(|_, ()| Ok(app.borrow().build().product_name.clone())),
    )?;
    put(
        table,
        "SetProductName",
        scope.create_function(|_, name: String| edit(app, |b| b.product_name = name)),
    )?;
    put(
        table,
        "GetStartupScene",
        scope.create_function(|_, ()| Ok(app.borrow().build().startup_scene.clone())),
    )?;
    put(
        table,
        "SetStartupScene",
        scope.create_function(|_, path: String| {
            if path.trim().is_empty() {
                return Err(mlua::Error::RuntimeError(
                    "Application.SetStartupScene: the path is empty".to_string(),
                ));
            }
            edit(app, |b| b.startup_scene = path)
        }),
    )
}

/// `Get/SetWindowMode` — `"Windowed"` or `"Fullscreen"`. An unknown name is ignored
/// and `SetWindowMode` returns `false`, like `Graphics.SetQuality`'s unknown tiers.
fn register_window_mode<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &Table,
    app: &'scope RefCell<Application>,
) -> Reg {
    put(
        table,
        "GetWindowMode",
        scope.create_function(|_, ()| Ok(app.borrow().build().window_mode.name())),
    )?;
    put(
        table,
        "SetWindowMode",
        scope.create_function(|_, name: String| {
            let Some(mode) = WindowMode::parse(&name) else {
                return Ok(false);
            };
            edit(app, |b| b.window_mode = mode)?;
            Ok(true)
        }),
    )
}
