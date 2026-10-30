//! `Harness.FindInDemo` / `SeekDemo` / `CutDemo` / `DemoPath` (#1597): a scenario
//! running on a demo (`play --demo <file> <scenario>`) finds a moment in its
//! index, replays to it, cuts a range of it into a scenario of its own, and asks
//! which demo it runs on.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use glam::Vec3;
use mlua::{Lua, Result as LuaResult, Table, Value};

use super::{world_of, Shared};
use crate::app::GameWorld;
use crate::dev::demo::{self, EventKind, Query, Replayer, NO_DEMO};

fn fail(verb: &str, msg: impl std::fmt::Display) -> mlua::Error {
    mlua::Error::RuntimeError(format!("Harness.{verb}: {msg}"))
}

/// The demo `world` replays: its file and how many ticks it holds, as the
/// replay read them.
fn on_demo(world: &Rc<RefCell<GameWorld>>, verb: &str) -> LuaResult<(PathBuf, u64)> {
    let mut world = world.borrow_mut();
    let replayer = world
        .tap_mut::<Replayer>()
        .ok_or_else(|| fail(verb, NO_DEMO))?;
    Ok((replayer.demo_path().to_path_buf(), replayer.verdict().of))
}

pub(super) fn register(lua: &Lua, harness: &Shared, t: &Table) -> LuaResult<()> {
    // FindInDemo(query) -> { tick, ... }: the ticks whose index matches, in order.
    let world = world_of(harness);
    t.set(
        "FindInDemo",
        lua.create_function(move |_, query: Table| {
            let query = query_of(&query)?;
            let (path, ticks) = on_demo(&world, "FindInDemo")?;
            Ok(demo::find_in(&path, ticks, &query))
        })?,
    )?;

    // SeekDemo(tick): the replay run up to and including `tick`.
    let world = world_of(harness);
    t.set(
        "SeekDemo",
        lua.create_function(move |_, tick: u64| {
            demo::seek(&mut world.borrow_mut(), tick).map_err(|e| fail("SeekDemo", e))
        })?,
    )?;

    // CutDemo(from, to, path): a scenario reproducing ticks `from` to `to`.
    let world = world_of(harness);
    t.set(
        "CutDemo",
        lua.create_function(move |_, (from, to, path): (u64, u64, String)| {
            let (demo, ticks) = on_demo(&world, "CutDemo")?;
            demo::cut(&demo, ticks, from, to, Path::new(&path)).map_err(|e| fail("CutDemo", e))
        })?,
    )?;

    // DemoPath() -> string | nil: the replayed demo's full path (a cut checks it).
    let world = world_of(harness);
    t.set(
        "DemoPath",
        lua.create_function(move |_, ()| {
            let path = on_demo(&world, "DemoPath").ok().map(|(path, _)| path);
            Ok(path.map(|p| demo::full_path(&p).to_string_lossy().into_owned()))
        })?,
    )
}

/// A `FindInDemo` query: one of `mark`, `log`, `event` or `inside`.
fn query_of(table: &Table) -> LuaResult<Query> {
    let mut queries = Vec::new();
    for pair in table.pairs::<String, Value>() {
        let (key, value) = pair?;
        let text = || -> LuaResult<String> {
            let text = value
                .as_string()
                .ok_or_else(|| fail("FindInDemo", format!("`{key}` must be a string")))?;
            Ok(text.to_str()?.to_string())
        };
        queries.push(match key.as_str() {
            "mark" => Query::Mark(text()?),
            "log" => Query::Log(text()?),
            "event" => Query::Event(EventKind::named(&text()?).map_err(|e| fail("FindInDemo", e))?),
            "inside" => inside(&value)?,
            other => {
                let msg = format!("no query {other:?}: `mark`, `log`, `event` or `inside`");
                return Err(fail("FindInDemo", msg));
            }
        });
    }
    match <[Query; 1]>::try_from(queries) {
        Ok([query]) => Ok(query),
        Err(_) => Err(fail(
            "FindInDemo",
            "a query holds exactly one of `mark`, `log`, `event` or `inside`",
        )),
    }
}

/// `inside = { min = {x, y, z}, max = {x, y, z} }`.
fn inside(value: &Value) -> LuaResult<Query> {
    let bad = || {
        fail(
            "FindInDemo",
            "`inside` is { min = {x, y, z}, max = {x, y, z} }",
        )
    };
    let table = value.as_table().ok_or_else(bad)?;
    let corner = |key: &str| -> LuaResult<Vec3> {
        let xyz: Vec<f32> = table.get(key).map_err(|_| bad())?;
        <[f32; 3]>::try_from(xyz)
            .map(Vec3::from_array)
            .map_err(|_| bad())
    };
    Ok(Query::Inside {
        min: corner("min")?,
        max: corner("max")?,
    })
}
