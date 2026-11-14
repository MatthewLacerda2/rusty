//! `Lighting.GetBakeReport(kind)` and `Navigation.GetBakeReport()` (#1980): the
//! last report a bake of the open scene wrote beside it (`dev::bake::report`), as
//! a table, so a scenario can assert on a bake ("no navmesh island under 2 m²",
//! "occlusion culls at least 40%"). `nil` when the scene is unsaved or that bake
//! never wrote one. Dev-only: the reports are diagnostics, never read at runtime.

use mlua::serde::SerializeOptions;
use mlua::{Lua, LuaSerdeExt, Value};

use super::super::bake::report::{read, Kind};
use rusty_script::api::{global_table, put, ApiCell, ApiCtx, Reg};

/// Add `GetBakeReport` to the `Lighting` and `Navigation` tables.
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

/// The open scene's last `kind` report as a Lua table, `nil` without one.
fn report(lua: &Lua, scene_path: ApiCell<Option<String>>, kind: Kind) -> mlua::Result<Value> {
    let path = scene_path.get(lua)?.borrow().clone();
    match path.and_then(|p| read(&p, kind)) {
        // A JSON `null` (a field the bake could not measure) reads as `nil`.
        Some(json) => {
            let nil = SerializeOptions::new()
                .serialize_none_to_null(false)
                .serialize_unit_to_null(false);
            lua.to_value_with(&json, nil)
        }
        None => Ok(Value::Nil),
    }
}
