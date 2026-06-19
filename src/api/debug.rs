//! src/api/debug.rs — `Debug` namespace (DEV-ONLY).
//!
//! `Debug.Log/Warn/Error` plus the structured scene-read `Debug.Snapshot` /
//! `Debug.SnapshotEntity` (#180). Registered only in dev builds — stripped from
//! the shipped game, like Unity `Debug.*` under `[Conditional]`. The whole module
//! is gated behind the `dev` feature in `api::mod`.

use mlua::Lua;

use super::{put, snapshot, ApiScopedCtx, Reg};

/// Register the `Debug` namespace onto `lua` (dev builds only).
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_logging(scope, &table, ctx.console)?;
    register_snapshot(scope, &table, ctx)?;

    lua.globals().set("Debug", table).map_err(|e| e.to_string())
}

/// `Debug.Log/Warn/Error` — append to the shared console buffer.
fn register_logging<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    console: &'scope std::cell::RefCell<crate::scripting::ConsoleLogs>,
) -> Reg {
    put(
        table,
        "Log",
        scope.create_function(|_, msg: String| {
            console.borrow_mut().info(msg);
            Ok(())
        }),
    )?;
    put(
        table,
        "Warn",
        scope.create_function(|_, msg: String| {
            console.borrow_mut().warn(msg);
            Ok(())
        }),
    )?;
    put(
        table,
        "Error",
        scope.create_function(|_, msg: String| {
            console.borrow_mut().error(msg);
            Ok(())
        }),
    )
}

/// `Debug.Snapshot()` (whole world) and `Debug.SnapshotEntity(id)` (one entity) —
/// the structured scene-read, returned as a pretty JSON string the agent parses.
fn register_snapshot<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    let scene = ctx.scene;
    let camera = ctx.camera;
    let time = ctx.time;
    let is_playing = ctx.is_playing;
    put(
        table,
        "Snapshot",
        scope.create_function(move |_, ()| {
            let value = snapshot::world_value(
                &scene.borrow(),
                &camera.borrow(),
                time.borrow().frame_count,
                *is_playing.borrow(),
            );
            Ok(serde_json::to_string_pretty(&value).unwrap_or_default())
        }),
    )?;

    let scene = ctx.scene;
    put(
        table,
        "SnapshotEntity",
        scope.create_function(move |_, id: u32| {
            let scene = scene.borrow();
            let world_matrix = scene.compute_world_matrix(id);
            let value = match scene.get_entity(id) {
                Some(entity) => {
                    let material = scene.material_of(&entity);
                    snapshot::entity_value(&entity, material, world_matrix)
                }
                None => serde_json::Value::Null,
            };
            Ok(serde_json::to_string_pretty(&value).unwrap_or_default())
        }),
    )
}
