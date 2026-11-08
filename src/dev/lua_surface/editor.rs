//! src/dev/lua_surface/editor.rs — `Editor` namespace (DEV-ONLY, #1360).
//!
//! `Editor.Undo()` / `Editor.Redo()` walk the edit-mode history (`scene::undo`, #1631),
//! Unity's `Undo.PerformUndo` / `PerformRedo`; `UndoName` / `RedoName` read it.
//!
//! `Editor.SetGizmo` / `Editor.GetGizmo` (#1909), Unity's
//! `GizmoUtility.SetGizmoEnabled`: the Scene view's gizmo toggles, by the names of
//! `core::application::Gizmo`'s table, through the same mailbox the demo verbs use
//! (`gizmo_verbs`).
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

use crate::api::ApiCell;
use crate::api::{put, ApiCtx, Reg};
use crate::core::application::{Application, DemoRequest, Gizmo};
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// Register the `Editor` namespace onto `lua` (dev builds only).
pub fn register(lua: &Lua, ctx: &ApiCtx) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    let (app, scene, console) = (ctx.application, ctx.scene, ctx.console);
    put(
        &table,
        "EnterPlay",
        lua.create_function(move |lua, ()| Ok(request(&*app.get(lua)?, &*console.get(lua)?, true))),
    )?;
    put(
        &table,
        "ExitPlay",
        lua.create_function(move |lua, ()| {
            Ok(request(&*app.get(lua)?, &*console.get(lua)?, false))
        }),
    )?;
    put(
        &table,
        "IsPlaying",
        lua.create_function(move |lua, ()| Ok(scene.get(lua)?.borrow().is_playing())),
    )?;
    undo_verbs(&table, lua, scene)?;
    demo_verbs(&table, lua, app, console)?;
    gizmo_verbs(&table, lua, app, console)?;
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

/// Undo / Redo (#1631): the same history as Ctrl+Z / Ctrl+Shift+Z and Edit ▸ Undo
/// (`scene::undo`), Unity's `Undo.PerformUndo` / `PerformRedo`.
fn undo_verbs(table: &mlua::Table, lua: &mlua::Lua, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "Undo",
        lua.create_function(move |lua, ()| Ok(scene.get(lua)?.borrow_mut().undo())),
    )?;
    put(
        table,
        "Redo",
        lua.create_function(move |lua, ()| Ok(scene.get(lua)?.borrow_mut().redo())),
    )?;
    put(
        table,
        "UndoName",
        lua.create_function(move |lua, ()| {
            Ok(scene.get(lua)?.borrow().undo_name().map(str::to_string))
        }),
    )?;
    put(
        table,
        "RedoName",
        lua.create_function(move |lua, ()| {
            Ok(scene.get(lua)?.borrow().redo_name().map(str::to_string))
        }),
    )?;
    Ok(())
}

/// The demo timeline's verbs (#1598), each what the timeline's own control does:
/// a request the editor window takes at the frame boundary. Only the editor window
/// has a timeline, so any other host refuses with a console warning.
fn demo_verbs(
    table: &mlua::Table,
    lua: &mlua::Lua,
    app: ApiCell<Application>,
    console: ApiCell<ConsoleLogs>,
) -> Reg {
    let ask = move |lua: &mlua::Lua, verb: &str, request: DemoRequest| -> mlua::Result<bool> {
        let app = app.get(lua)?;
        let mut app = app.borrow_mut();
        if !app.demo_viewer() {
            console.get(lua)?.borrow_mut().warn(format!(
                "Editor.{verb}: refused: only the editor window has a demo timeline"
            ));
            return Ok(false);
        }
        app.request_demo(request);
        Ok(true)
    };
    put(
        table,
        "OpenDemo",
        lua.create_function(move |lua, path: String| ask(lua, "OpenDemo", DemoRequest::Open(path))),
    )?;
    put(
        table,
        "SeekDemo",
        lua.create_function(move |lua, tick: u64| ask(lua, "SeekDemo", DemoRequest::Seek(tick))),
    )?;
    put(
        table,
        "PauseDemo",
        lua.create_function(move |lua, paused: Option<bool>| {
            ask(lua, "PauseDemo", DemoRequest::Pause(paused.unwrap_or(true)))
        }),
    )?;
    put(
        table,
        "DemoTick",
        lua.create_function(move |lua, ()| {
            Ok(app.get(lua)?.borrow().demo_tick().map(|(tick, _)| tick))
        }),
    )?;
    put(
        table,
        "DemoLength",
        lua.create_function(move |lua, ()| {
            Ok(app.get(lua)?.borrow().demo_tick().map(|(_, of)| of))
        }),
    )
}

/// The Scene view's gizmo toggles (#1909), Unity's `GizmoUtility.SetGizmoEnabled`:
/// `SetGizmo` lands in the shared `Application` (read back at once) and the editor
/// window takes it at the frame boundary. Only the editor window has a Scene view, so
/// any other host refuses a set; an unknown name warns.
fn gizmo_verbs(
    table: &mlua::Table,
    lua: &mlua::Lua,
    app: ApiCell<Application>,
    console: ApiCell<ConsoleLogs>,
) -> Reg {
    let named = move |lua: &mlua::Lua, verb: &str, name: &str| -> mlua::Result<Option<Gizmo>> {
        let gizmo = Gizmo::named(name);
        if gizmo.is_none() {
            let names: Vec<_> = Gizmo::ALL.iter().map(|g| g.row().name).collect();
            console.get(lua)?.borrow_mut().warn(format!(
                "Editor.{verb}: no gizmo {name:?} (one of {})",
                names.join(", ")
            ));
        }
        Ok(gizmo)
    };
    put(
        table,
        "SetGizmo",
        lua.create_function(move |lua, (name, on): (String, bool)| {
            let Some(gizmo) = named(lua, "SetGizmo", &name)? else {
                return Ok(false);
            };
            let app = app.get(lua)?;
            let mut app = app.borrow_mut();
            if !app.gizmo_viewer() {
                console.get(lua)?.borrow_mut().warn(
                    "Editor.SetGizmo: refused: only the editor window has a Scene view".to_string(),
                );
                return Ok(false);
            }
            app.request_gizmo(gizmo, on);
            Ok(true)
        }),
    )?;
    put(
        table,
        "GetGizmo",
        lua.create_function(move |lua, name: String| {
            let Some(gizmo) = named(lua, "GetGizmo", &name)? else {
                return Ok(None);
            };
            Ok(Some(app.get(lua)?.borrow().scene_gizmos().on(gizmo)))
        }),
    )
}
