//! `Lighting.GenerateScenes` and its queue verbs: baked in one call, or by the editor.

use std::cell::RefCell;
use std::path::Path;

use mlua::{Lua, Table, Value};

use crate::bake::progress::BakeLog;
use crate::bake::queue::live::{live_files, refuse, run_beside};
use crate::bake::queue::{read, resolve, Owner, Queue, Read, Resumed, Runner, Selection};
use rusty_scene::lighting::bake_status::BakeStatus;
use rusty_scene::Scene;
use rusty_script::api::{global_table, put, ApiCtx, Reg};
use rusty_script::scripting::ConsoleLogs;

pub const NO_QUEUE_HOST: &str = "Lighting.GenerateScenesAsync: no editor frame loop here to \
run the bake queue in the background (a headless session or a standalone run); call \
Lighting.GenerateScenes() instead, which bakes them in the one call";

/// Opening the project made it the working directory.
fn root() -> &'static Path {
    Path::new(".")
}

pub(super) fn register(lua: &Lua, ctx: &ApiCtx) -> Reg {
    let table = global_table(lua, "Lighting")?;
    register_generate(lua, &table, ctx)?;
    register_async(lua, &table, ctx)?;
    register_resume(lua, &table, ctx)?;
    let console = ctx.console;
    put(
        &table,
        "GetBakeQueue",
        lua.create_function(move |lua, ()| match read(root()) {
            Read::Missing => Ok(Value::Nil),
            Read::Unreadable(why) => {
                console
                    .get(lua)?
                    .borrow_mut()
                    .warn(format!("[Lighting] {why}"));
                Ok(Value::Nil)
            }
            Read::Found(queue) => {
                let out = queue_table(lua, &queue)?;
                out.set("owner", queue.owner)?;
                out.set("running", Owner::is_held(root()))?;
                out.set("unfinished", queue.unfinished())?;
                Ok(Value::Table(out))
            }
        }),
    )
}

fn register_generate(lua: &Lua, table: &Table, ctx: &ApiCtx) -> Reg {
    let (scene, scene_path, app, console) =
        (ctx.scene, ctx.scene_path, ctx.application, ctx.console);
    put(
        table,
        "GenerateScenes",
        lua.create_function(move |lua, which: Value| {
            let selection = selection("GenerateScenes", which)?;
            let (scene, path) = (&*scene.get(lua)?, &*scene_path.get(lua)?);
            let open = path.borrow().clone();
            let heading = app.get(lua)?.borrow().play_heading();
            refuse(&scene.borrow(), open.as_deref(), heading).map_err(runtime)?;
            let paths = resolve(root(), &selection);
            let mut runner = Runner::start(root(), paths).map_err(named("GenerateScenes"))?;
            run_live(lua, &mut runner, (scene, open), &*console.get(lua)?)
        }),
    )
}

fn run_live(
    lua: &Lua,
    runner: &mut Runner,
    (scene, open): (&RefCell<Scene>, Option<String>),
    console: &RefCell<ConsoleLogs>,
) -> mlua::Result<Table> {
    let mut say = |line: String| console.borrow_mut().info(line);
    let mut log = BakeLog::to_console(console);
    let several = scene.borrow().loaded.active_of_several().is_some();
    let files = live_files(&scene.borrow(), open.as_deref());
    let reopen = run_beside(runner, &files, &mut say, &mut log).map_err(runtime)?;
    drop(log);
    let summary = runner.summary();
    console.borrow_mut().info(summary);
    if reopen && several {
        let warnings = rusty_edit::scenes::reopen_loaded(&mut scene.borrow_mut());
        for line in warnings.map_err(runtime)? {
            console.borrow_mut().warn(line);
        }
    } else if let (true, Some(open)) = (reopen, &open) {
        match rusty_edit::load_from_file(&mut scene.borrow_mut(), open) {
            Ok(()) => {}
            Err(e) if e.replaced_world() => console.borrow_mut().warn(format!("{open}: {e}")),
            Err(e) => return Err(runtime(format!("reopening {open}: {e}"))),
        }
    }
    queue_table(lua, runner.queue())
}

fn register_async(lua: &Lua, table: &Table, ctx: &ApiCtx) -> Reg {
    let (scene, scene_path, app) = (ctx.scene, ctx.scene_path, ctx.application);
    put(
        table,
        "GenerateScenesAsync",
        lua.create_function(move |lua, which: Value| {
            let selection = selection("GenerateScenesAsync", which)?;
            if !app.get(lua)?.borrow().bake_host() {
                return Err(runtime(NO_QUEUE_HOST.into()));
            }
            let (scene, path) = (&*scene.get(lua)?, &*scene_path.get(lua)?);
            let heading = app.get(lua)?.borrow().play_heading();
            let open = path.borrow().clone();
            let open = open.ok_or_else(|| runtime(UNSAVED_ASYNC.into()))?;
            refuse(&scene.borrow(), Some(&open), heading).map_err(runtime)?;
            // Let go: the editor takes it from the file.
            let paths = resolve(root(), &selection);
            drop(Runner::start(root(), paths).map_err(named("GenerateScenesAsync"))?);
            ask_editor(&mut scene.borrow_mut());
            Ok(true)
        }),
    )
}

const UNSAVED_ASYNC: &str = "Lighting.GenerateScenesAsync: save the scene first";

fn ask_editor(scene: &mut Scene) {
    scene.lighting_queue_requested = true;
    scene.lighting_bake_status = BakeStatus::Requested;
    rusty_core::wake::frame();
}

fn register_resume(lua: &Lua, table: &Table, ctx: &ApiCtx) -> Reg {
    let (scene, scene_path, app, console) =
        (ctx.scene, ctx.scene_path, ctx.application, ctx.console);
    put(
        table,
        "ResumeBakeQueue",
        lua.create_function(move |lua, ()| {
            let (scene, path) = (&*scene.get(lua)?, &*scene_path.get(lua)?);
            let open = path.borrow().clone();
            let app = app.get(lua)?;
            let heading = app.borrow().play_heading();
            let out = lua.create_table()?;
            if let Some(nothing) = nothing_to_resume() {
                out.set("resumed", false)?;
                out.set("message", nothing)?;
                return Ok(out);
            }
            refuse(&scene.borrow(), open.as_deref(), heading).map_err(runtime)?;
            out.set("resumed", true)?;
            if app.borrow().bake_host() {
                if open.is_none() {
                    return Err(runtime(UNSAVED_ASYNC.into()));
                }
                if Owner::is_held(root()) {
                    return Err(runtime(Owner::claim(root()).err().unwrap_or_default()));
                }
                ask_editor(&mut scene.borrow_mut());
                out.set("message", "the editor continues the bake queue")?;
                return Ok(out);
            }
            let mut runner = match Runner::resume(root()).map_err(runtime)? {
                Resumed::Runner(runner) => runner,
                Resumed::Nothing(why) => {
                    out.set("resumed", false)?;
                    out.set("message", why)?;
                    return Ok(out);
                }
            };
            let scenes = run_live(lua, &mut runner, (scene, open), &*console.get(lua)?)?;
            out.set("message", runner.summary())?;
            out.set("scenes", scenes.get::<Table>("scenes")?)?;
            Ok(out)
        }),
    )
}

fn nothing_to_resume() -> Option<String> {
    match read(root()) {
        Read::Found(queue) if queue.unfinished() => None,
        Read::Found(_) => Some("the bake queue is finished".into()),
        Read::Missing => Some("there is no bake queue".into()),
        Read::Unreadable(why) => Some(why),
    }
}

fn queue_table(lua: &Lua, queue: &Queue) -> mlua::Result<Table> {
    let scenes = lua.create_table()?;
    for (i, e) in queue.scenes.iter().enumerate() {
        let row = lua.create_table()?;
        row.set("path", e.path.as_str())?;
        row.set("state", e.state.name())?;
        row.set("reason", e.reason.as_deref())?;
        row.set("attempts", e.attempts)?;
        row.set("startedMs", e.started_ms)?;
        row.set("finishedMs", e.finished_ms)?;
        scenes.set(i + 1, row)?;
    }
    let out = lua.create_table()?;
    out.set("scenes", scenes)?;
    out.set("failed", queue.failed())?;
    Ok(out)
}

pub(super) fn selection(verb: &'static str, which: Value) -> mlua::Result<Selection> {
    let bad = || {
        named(verb)("pass a list of scene paths, \"all\", or { all = true, except = { … } }".into())
    };
    match which {
        Value::String(s) if s.to_str()?.as_ref() == "all" => Ok(Selection::All { except: vec![] }),
        Value::Table(t) => match t.get::<Option<bool>>("all")? {
            Some(true) => Ok(Selection::All {
                except: t.get::<Option<Vec<String>>>("except")?.unwrap_or_default(),
            }),
            Some(false) => Err(bad()),
            // `except` without `all = true` is a forgotten `all`, not an empty list.
            None if t.contains_key("except")? => Err(bad()),
            None => {
                let paths: Vec<String> = t.sequence_values().collect::<Result<_, _>>()?;
                if paths.is_empty() {
                    return Err(bad());
                }
                Ok(Selection::List(paths))
            }
        },
        _ => Err(bad()),
    }
}

/// The queue's refusal, under the name of the verb the script called.
fn named(verb: &'static str) -> impl Fn(String) -> mlua::Error {
    move |why| runtime(format!("Lighting.{verb}: {why}"))
}

fn runtime(message: String) -> mlua::Error {
    mlua::Error::RuntimeError(message)
}

#[cfg(test)]
#[path = "queue_tests.rs"]
mod tests;
