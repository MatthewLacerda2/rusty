// sorted: one module per dev namespace; kept in order so parallel branches don't collide
mod bake;
mod bake_report;
mod capture;
mod debug;
mod editor;
mod game_view;
mod generate;

use std::sync::Once;

use mlua::Lua;

use rusty_script::api::{self, ApiCtx, Reg};

pub fn install_api() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| api::extend(register));
}

fn register(lua: &Lua, ctx: &ApiCtx) -> Reg {
    // sorted: each registrar adds its own table or extends one `api` made
    bake::register(lua, ctx)?;
    bake_report::register(lua, ctx)?;
    capture::register(lua, ctx)?;
    debug::register(lua, ctx)?;
    editor::register(lua, ctx)?;
    generate::register(lua, ctx)
}

#[cfg(test)]
mod bake_report_tests;
#[cfg(test)]
mod editor_tests;
#[cfg(test)]
mod game_view_tests;
#[cfg(test)]
mod gizmo_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod undo_tests;
