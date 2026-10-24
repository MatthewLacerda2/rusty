//! src/dev/lua_surface/editor.rs — `Editor` namespace (DEV-ONLY, #1360).
//!
//! Unity's `EditorApplication.isPlaying`: `Editor.EnterPlay()` / `Editor.ExitPlay()`
//! make the same transition the Play/Stop button makes, and `Editor.IsPlaying()` reads
//! the play state. A call only *requests* the transition (on the shared
//! [`Application`](crate::core::application::Application) cell); the host takes it at
//! the next frame boundary, so Stop restores the edit-mode snapshot exactly as the
//! button does, and a script never tears its own VM down mid-call. Dev builds only,
//! like `Debug`: a shipped game has no editor to switch.

use std::cell::RefCell;

use mlua::Lua;

use crate::api::{put, ApiScopedCtx, Reg};
use crate::core::application::Application;
use crate::scene::Scene;

/// Register the `Editor` namespace onto `lua` (dev builds only).
pub fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    let (app, scene) = (ctx.application, ctx.scene);
    put(
        &table,
        "EnterPlay",
        scope.create_function(move |_, ()| Ok(request(app, scene, true))),
    )?;
    put(
        &table,
        "ExitPlay",
        scope.create_function(move |_, ()| Ok(request(app, scene, false))),
    )?;
    put(
        &table,
        "IsPlaying",
        scope.create_function(move |_, ()| Ok(scene.borrow().is_playing())),
    )?;
    lua.globals()
        .set("Editor", table)
        .map_err(|e| e.to_string())
}

/// Ask for `playing`, returning whether that is a change: the state it is heading to
/// (a request not yet taken, else the scene's) already being `playing` is a no-op.
fn request(app: &RefCell<Application>, scene: &RefCell<Scene>, playing: bool) -> bool {
    let mut app = app.borrow_mut();
    let heading = app
        .play_request()
        .unwrap_or_else(|| scene.borrow().is_playing());
    if heading == playing {
        return false;
    }
    app.request_play(playing);
    true
}
