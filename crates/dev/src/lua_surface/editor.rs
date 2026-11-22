//! A Play request lands on `Application` and the host takes it at the next frame boundary,
//! so a script never tears its own VM down mid-call.

use std::cell::RefCell;

use mlua::Lua;

use rusty_core::application::{Application, DemoRequest, Gizmo};
use rusty_scene::Scene;
use rusty_script::api::ApiCell;
use rusty_script::api::{put, ApiCtx, Reg};
use rusty_script::scripting::ConsoleLogs;

pub fn register(lua: &Lua, ctx: &ApiCtx) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    let (app, scene, console) = (ctx.application, ctx.scene, ctx.console);
    put(
        &table,
        "EnterPlay",
        lua.create_function(move |lua, ()| {
            let (app, console) = (app.get(lua)?, console.get(lua)?);
            // A no-op (standalone, or already heading to Play) answers `false`, never a refusal.
            let changes = !app.borrow().standalone() && !app.borrow().play_heading();
            let refusal = scene.get(lua)?.borrow().play_refusal();
            if let (true, Some(why)) = (changes, refusal) {
                return Err(mlua::Error::runtime(format!("Editor.EnterPlay: {why}")));
            }
            Ok(request(&app, &console, true))
        }),
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
    probe_verbs(&table, lua, scene)?;
    demo_verbs(&table, lua, app, console)?;
    gizmo_verbs(&table, lua, app, console)?;
    super::game_view::verbs(&table, lua, app, console)?;
    lua.globals()
        .set("Editor", table)
        .map_err(|e| e.to_string())
}

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

fn probe_verbs(table: &mlua::Table, lua: &mlua::Lua, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "InspectReflectionProbe",
        lua.create_function(move |lua, index: usize| {
            let scene = scene.get(lua)?;
            let inspected = scene.borrow_mut().inspect_reflection_probe(index);
            let why = |e| mlua::Error::runtime(format!("Editor.InspectReflectionProbe: {e}"));
            inspected.map_err(why)
        }),
    )?;
    put(
        table,
        "ClearInspectedReflectionProbe",
        lua.create_function(move |lua, ()| {
            scene.get(lua)?.borrow_mut().inspected_reflection_probe = None;
            Ok(())
        }),
    )?;
    put(
        table,
        "InspectedReflectionProbe",
        lua.create_function(move |lua, ()| Ok(scene.get(lua)?.borrow().inspected_reflection_probe)),
    )?;
    Ok(())
}

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
