use super::scene_load::{check_file, load_now};
use super::{put, Reg};
use crate::api::ApiCell;
use rusty_scene::runtime::loads::LoadStatus;
use rusty_scene::Scene;

pub fn register(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    scene_path: ApiCell<Option<String>>,
) -> Reg {
    register_load_async(lua, table, scene, scene_path)?;
    let status = move |lua: &mlua::Lua, op: u32| -> mlua::Result<Option<LoadStatus>> {
        Ok(scene.get(lua)?.borrow().loads.status(op))
    };
    put(
        table,
        "GetLoadProgress",
        lua.create_function(move |lua, op: u32| {
            Ok(scene.get(lua)?.borrow().loads.progress(op).unwrap_or(0.0))
        }),
    )?;
    put(
        table,
        "IsLoadDone",
        lua.create_function(move |lua, op: u32| {
            Ok(status(lua, op)?.is_none_or(LoadStatus::is_done))
        }),
    )?;
    put(
        table,
        "GetLoadStatus",
        lua.create_function(move |lua, op: u32| {
            Ok(status(lua, op)?.map_or("gone", LoadStatus::name))
        }),
    )?;
    put(
        table,
        "SetAllowSceneActivation",
        lua.create_function(move |lua, (op, allow): (u32, bool)| {
            Ok(scene
                .get(lua)?
                .borrow_mut()
                .loads
                .allow_activation(op, allow))
        }),
    )
}

fn register_load_async(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    scene_path: ApiCell<Option<String>>,
) -> Reg {
    put(
        table,
        "LoadAsync",
        lua.create_function(move |lua, path: String| {
            let scene = scene.get(lua)?;
            check_file("LoadAsync", &path)?;
            if scene.borrow().is_playing() {
                return Ok(scene.borrow_mut().loads.request_async(path));
            }
            if let Some(warning) = load_now("LoadAsync", &scene, &*scene_path.get(lua)?, path)? {
                log::warn!("[Scene] {warning}");
            }
            let op = scene.borrow_mut().loads.ended(LoadStatus::Done);
            Ok(op)
        }),
    )
}
