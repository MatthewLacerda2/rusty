use std::cell::RefCell;

use mlua::{Lua, Table};

use super::{put, Reg};
use crate::api::ApiCell;
use rusty_core::application::{Application, LoadingPriority, WindowMode};

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
    register_loading_priority(lua, &table, app)?;

    lua.globals()
        .set("Application", table)
        .map_err(|e| e.to_string())
}

fn edit(
    app: &RefCell<Application>,
    edit: impl FnOnce(&mut rusty_core::application::BuildSettings),
) -> mlua::Result<()> {
    app.borrow_mut()
        .update(edit)
        .map_err(mlua::Error::RuntimeError)
}

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

fn register_loading_priority(lua: &mlua::Lua, table: &Table, app: ApiCell<Application>) -> Reg {
    put(
        table,
        "GetBackgroundLoadingPriority",
        lua.create_function(move |lua, ()| {
            Ok(app.get(lua)?.borrow().background_loading_priority().name())
        }),
    )?;
    put(
        table,
        "SetBackgroundLoadingPriority",
        lua.create_function(move |lua, name: String| {
            let Some(priority) = LoadingPriority::parse(&name) else {
                return Ok(false);
            };
            let app = app.get(lua)?;
            app.borrow_mut().set_background_loading_priority(priority);
            Ok(true)
        }),
    )
}
