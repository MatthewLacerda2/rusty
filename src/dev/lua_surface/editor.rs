//! src/dev/lua_surface/editor.rs — `Editor` namespace (DEV-ONLY, #1360).
//!
//! `Editor.Undo()` / `Editor.Redo()` walk the edit-mode history (`scene::undo`, #1631),
//! Unity's `Undo.PerformUndo` / `PerformRedo`; `UndoName` / `RedoName` read it.
//!
//! Unity's `EditorApplication.isPlaying`: `Editor.EnterPlay()` / `Editor.ExitPlay()`
//! make the same transition the Play/Stop button makes, and `Editor.IsPlaying()` reads
//! the play state. A call only *requests* the transition (on the shared
//! [`Application`](crate::core::application::Application) cell); the host takes it at
//! the next frame boundary, so Stop restores the edit-mode snapshot exactly as the
//! button does, and a script never tears its own VM down mid-call. Dev builds only,
//! like `Debug`: a shipped game has no editor to switch, and a dev-built standalone run
//! (the `player`, the `play` harness) refuses the request with a console warning, as
//! Unity's `EditorApplication` doesn't exist in a player.

use std::cell::RefCell;

use mlua::Lua;

use crate::api::{put, ApiScopedCtx, Reg};
use crate::core::application::Application;
use crate::scripting::ConsoleLogs;

/// Register the `Editor` namespace onto `lua` (dev builds only).
pub fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    let (app, scene, console) = (ctx.application, ctx.scene, ctx.console);
    put(
        &table,
        "EnterPlay",
        scope.create_function(move |_, ()| Ok(request(app, console, true))),
    )?;
    put(
        &table,
        "ExitPlay",
        scope.create_function(move |_, ()| Ok(request(app, console, false))),
    )?;
    put(
        &table,
        "IsPlaying",
        scope.create_function(move |_, ()| Ok(scene.borrow().is_playing())),
    )?;
    // Undo / Redo (#1631): the same history as Ctrl+Z / Ctrl+Shift+Z and Edit ▸ Undo.
    put(
        &table,
        "Undo",
        scope.create_function(move |_, ()| Ok(scene.borrow_mut().undo())),
    )?;
    put(
        &table,
        "Redo",
        scope.create_function(move |_, ()| Ok(scene.borrow_mut().redo())),
    )?;
    put(
        &table,
        "UndoName",
        scope.create_function(move |_, ()| Ok(scene.borrow().undo_name().map(str::to_string))),
    )?;
    put(
        &table,
        "RedoName",
        scope.create_function(move |_, ()| Ok(scene.borrow().redo_name().map(str::to_string))),
    )?;
    lua.globals()
        .set("Editor", table)
        .map_err(|e| e.to_string())
}

/// Ask for `playing`, returning whether that is a change: the state the next
/// transition already heads to (a script's request not yet taken, else the host's,
/// e.g. a Play button pressed this frame) being `playing` is a no-op. A standalone
/// run refuses, with a console warning: it has no edit mode to switch to or from.
fn request(app: &RefCell<Application>, console: &RefCell<ConsoleLogs>, playing: bool) -> bool {
    let mut app = app.borrow_mut();
    if app.standalone() {
        let verb = if playing { "EnterPlay" } else { "ExitPlay" };
        console.borrow_mut().warn(format!(
            "Editor.{verb}: refused in a standalone run (the player, the harness): \
             there is no edit mode to switch"
        ));
        return false;
    }
    if app.play_heading() == playing {
        return false;
    }
    app.request_play(playing);
    true
}
