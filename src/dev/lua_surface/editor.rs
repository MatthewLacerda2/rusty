//! src/dev/lua_surface/editor.rs — `Editor` namespace (DEV-ONLY, #1360).
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
use crate::core::application::{Application, DemoRequest};
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
    demo_verbs(&table, scope, app, console)?;
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

/// The demo timeline's verbs (#1598), each what the timeline's own control does:
/// a request the editor window takes at the frame boundary. Only the editor window
/// has a timeline, so any other host refuses with a console warning.
fn demo_verbs<'scope>(
    table: &mlua::Table,
    scope: &'scope mlua::Scope<'scope, '_>,
    app: &'scope RefCell<Application>,
    console: &'scope RefCell<ConsoleLogs>,
) -> Reg {
    let ask = move |verb: &str, request: DemoRequest| -> bool {
        let mut app = app.borrow_mut();
        if !app.demo_viewer() {
            console.borrow_mut().warn(format!(
                "Editor.{verb}: refused: only the editor window has a demo timeline"
            ));
            return false;
        }
        app.request_demo(request);
        true
    };
    put(
        table,
        "OpenDemo",
        scope.create_function(move |_, path: String| Ok(ask("OpenDemo", DemoRequest::Open(path)))),
    )?;
    put(
        table,
        "SeekDemo",
        scope.create_function(move |_, tick: u64| Ok(ask("SeekDemo", DemoRequest::Seek(tick)))),
    )?;
    put(
        table,
        "PauseDemo",
        scope.create_function(move |_, paused: Option<bool>| {
            Ok(ask("PauseDemo", DemoRequest::Pause(paused.unwrap_or(true))))
        }),
    )?;
    put(
        table,
        "DemoTick",
        scope.create_function(move |_, ()| Ok(app.borrow().demo_tick().map(|(tick, _)| tick))),
    )?;
    put(
        table,
        "DemoLength",
        scope.create_function(move |_, ()| Ok(app.borrow().demo_tick().map(|(_, of)| of))),
    )
}
