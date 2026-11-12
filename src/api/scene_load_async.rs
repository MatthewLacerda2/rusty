//! src/api/scene_load_async.rs — `Scene.LoadAsync` and its operation (#2123), Unity's
//! `SceneManager.LoadSceneAsync` and `AsyncOperation`.
//!
//! Registered onto the `Scene` table with `api::scene_load`. rusty's namespaces hand
//! out ids, not objects, so the operation is an integer polled through namespace
//! verbs, as `EnvironmentQuery.Request`'s ticket is (`docs/api/Scene.md`):
//!
//!   - `LoadAsync(path)` — open an operation; in edit mode the scene loads at once.
//!   - `GetLoadProgress(op)` / `IsLoadDone(op)` — `progress` and `isDone`.
//!   - `GetLoadStatus(op)` — `"loading"`, `"ready"`, `"done"`, `"failed"`,
//!     `"replaced"`, or `"gone"` for an id this Play session never opened.
//!   - `SetAllowSceneActivation(op, allow)` — `allowSceneActivation`.
//!
//! The state they read is the scene's (`scene::runtime::loads`), moved only at a
//! tick's head and tail, so a script reads the same values on every replay.

use super::scene_load::{check_file, load_now};
use super::{put, Reg};
use crate::api::ApiCell;
use crate::scene::runtime::loads::LoadStatus;
use crate::scene::Scene;

/// Register `LoadAsync` and its operation's verbs onto the `Scene` `table`.
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
            Ok(status(lua, op)?.map_or(0.0, LoadStatus::progress))
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

/// `LoadAsync(path)`: an operation in Play; in edit mode the scene loads at once and
/// the operation is born done.
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
            if let Some(warning) = load_now(&scene, &*scene_path.get(lua)?, path)? {
                log::warn!("[Scene] {warning}");
            }
            let op = scene.borrow_mut().loads.ended(LoadStatus::Done);
            Ok(op)
        }),
    )
}
