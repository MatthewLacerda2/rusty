use std::path::Path;

use mlua::Lua;

use rusty_script::api::{global_table, put, ApiCtx, Reg};

pub(super) fn register(lua: &Lua, ctx: &ApiCtx) -> Reg {
    let table = global_table(lua, "Audio")?;
    let audio = ctx.audio;
    put(
        &table,
        "StartCapture",
        lua.create_function(move |lua, path: String| {
            let started = audio.get(lua)?.borrow_mut().start_capture(Path::new(&path));
            started.map_err(mlua::Error::RuntimeError)
        }),
    )?;
    put(
        &table,
        "StopCapture",
        lua.create_function(move |lua, ()| {
            let path = audio.get(lua)?.borrow_mut().stop_capture();
            let path = path.map_err(mlua::Error::RuntimeError)?;
            Ok(rusty_core::paths::to_slash(&path))
        }),
    )
}
