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
//! `project.rusty`; in the player and the harness the cell is unbound,
//! so a setter changes the in-memory value only.

use std::cell::RefCell;

use mlua::{Lua, Table};

use super::{put, Reg};
use crate::api::ApiCell;
use crate::core::application::{Application, WindowMode};

/// Register the `Application` namespace onto `lua`, backed by the shared cell.
pub fn register(lua: &Lua, app: ApiCell<Application>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "Quit",
        lua.create_function(move |lua, ()| {
            app.get(lua)?.borrow_mut().request_quit();
            Ok(())
        }),
    )?;
    register_names(lua, &table, app)?;
    register_window_mode(lua, &table, app)?;

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
fn register_names(lua: &mlua::Lua, table: &Table, app: ApiCell<Application>) -> Reg {
    put(
        table,
        "GetProductName",
        lua.create_function(move |lua, ()| Ok(app.get(lua)?.borrow().build().product_name.clone())),
    )?;
    put(
        table,
        "SetProductName",
        lua.create_function(move |lua, name: String| {
            edit(&*app.get(lua)?, |b| b.product_name = name)
        }),
    )?;
    put(
        table,
        "GetStartupScene",
        lua.create_function(move |lua, ()| {
            Ok(app.get(lua)?.borrow().build().startup_scene.clone())
        }),
    )?;
    put(
        table,
        "SetStartupScene",
        lua.create_function(move |lua, path: String| {
            if path.trim().is_empty() {
                return Err(mlua::Error::RuntimeError(
                    "Application.SetStartupScene: the path is empty".to_string(),
                ));
            }
            edit(&*app.get(lua)?, |b| b.startup_scene = path)
        }),
    )
}

/// `Get/SetWindowMode` — `"Windowed"` or `"Fullscreen"`. An unknown name is ignored
/// and `SetWindowMode` returns `false`, like `Graphics.SetQuality`'s unknown tiers.
fn register_window_mode(lua: &mlua::Lua, table: &Table, app: ApiCell<Application>) -> Reg {
    put(
        table,
        "GetWindowMode",
        lua.create_function(move |lua, ()| Ok(app.get(lua)?.borrow().build().window_mode.name())),
    )?;
    put(
        table,
        "SetWindowMode",
        lua.create_function(move |lua, name: String| {
            let Some(mode) = WindowMode::parse(&name) else {
                return Ok(false);
            };
            edit(&*app.get(lua)?, |b| b.window_mode = mode)?;
            Ok(true)
        }),
    )
}
