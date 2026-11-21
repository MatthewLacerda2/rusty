use mlua::serde::SerializeOptions;
use mlua::{Lua, LuaSerdeExt, Value};

use super::super::bake::report::{read, Kind};
use rusty_script::api::{global_table, put, ApiCell, ApiCtx, Reg};

pub(super) fn register(lua: &Lua, ctx: &ApiCtx) -> Reg {
    let scene_path = ctx.scene_path;
    put(
        &global_table(lua, "Lighting")?,
        "GetBakeReport",
        lua.create_function(move |lua, kind: String| {
            let lighting = Kind::parse(&kind).filter(|k| *k != Kind::Navmesh);
            let Some(kind) = lighting else {
                return Err(mlua::Error::runtime(format!(
                    "Lighting.GetBakeReport: unknown kind {kind:?} (lightmaps, probes, \
                     reflections or occlusion)"
                )));
            };
            report(lua, scene_path, kind)
        }),
    )?;
    put(
        &global_table(lua, "Navigation")?,
        "GetBakeReport",
        lua.create_function(move |lua, ()| report(lua, scene_path, Kind::Navmesh)),
    )
}

fn report(lua: &Lua, scene_path: ApiCell<Option<String>>, kind: Kind) -> mlua::Result<Value> {
    let path = scene_path.get(lua)?.borrow().clone();
    match path.and_then(|p| read(&p, kind)) {
        Some(json) => {
            let nil = SerializeOptions::new()
                .serialize_none_to_null(false)
                .serialize_unit_to_null(false);
            lua.to_value_with(&json, nil)
        }
        None => Ok(Value::Nil),
    }
}
