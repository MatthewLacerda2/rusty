use super::super::{put, Reg};
use crate::api::ApiCell;
use rusty_core::application::Application;
use rusty_core::settings::graphics_options::GraphicsOption;
use rusty_core::settings::GraphicsSettings;

fn option(call: &str, name: &str) -> mlua::Result<GraphicsOption> {
    GraphicsOption::parse(name).ok_or_else(|| {
        mlua::Error::RuntimeError(format!(
            "Graphics.{call}: unknown option '{name}' (one of {})",
            GraphicsOption::names()
        ))
    })
}

pub(super) fn register_options(
    lua: &mlua::Lua,
    table: &mlua::Table,
    graphics: ApiCell<GraphicsSettings>,
    app: ApiCell<Application>,
) -> Reg {
    put(
        table,
        "GetOption",
        lua.create_function(move |lua, name: String| {
            let option = option("GetOption", &name)?;
            Ok(graphics.get(lua)?.borrow().options.allows(option))
        }),
    )?;
    put(
        table,
        "SetOption",
        lua.create_function(move |lua, (name, on): (String, bool)| {
            let option = option("SetOption", &name)?;
            graphics.get(lua)?.borrow_mut().options.choose(option, on);
            Ok(())
        }),
    )?;
    put(
        table,
        "ResetOptions",
        lua.create_function(move |lua, ()| {
            let defaults = app.get(lua)?.borrow().graphics_defaults();
            graphics.get(lua)?.borrow_mut().options.reset(defaults);
            Ok(())
        }),
    )?;
    put(
        table,
        "GetOptionNames",
        lua.create_function(|_, ()| Ok(GraphicsOption::ALL.map(GraphicsOption::name).to_vec())),
    )?;
    register_defaults(lua, table, graphics, app)
}

fn register_defaults(
    lua: &mlua::Lua,
    table: &mlua::Table,
    graphics: ApiCell<GraphicsSettings>,
    app: ApiCell<Application>,
) -> Reg {
    put(
        table,
        "GetDefaultOption",
        lua.create_function(move |lua, name: String| {
            let option = option("GetDefaultOption", &name)?;
            Ok(app.get(lua)?.borrow().graphics_defaults().allows(option))
        }),
    )?;
    put(
        table,
        "SetDefaultOption",
        lua.create_function(move |lua, (name, on): (String, bool)| {
            graphics.with(lua, |graphics| {
                let option = option("SetDefaultOption", &name)?;
                let live = &mut graphics.borrow_mut().options;
                app.get(lua)?
                    .borrow_mut()
                    .set_default_option(option, on, live)
                    .map_err(mlua::Error::RuntimeError)
            })
        }),
    )
}
