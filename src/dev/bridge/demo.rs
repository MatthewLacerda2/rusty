//! `Harness.FindInDemo` / `SeekDemo` / `CutDemo` (#1597): a scenario running on a
//! demo (`play --demo <file> <scenario>`) finds a moment in its index, replays to
//! it, and cuts a range of it into a scenario of its own.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use glam::Vec3;
use mlua::{Lua, Result as LuaResult, Table, Value};

use super::{world_of, Shared};
use crate::app::GameWorld;
use crate::dev::demo::{self, Demo, EventKind, Query, Replayer, NO_DEMO};

fn fail(verb: &str, msg: impl std::fmt::Display) -> mlua::Error {
    mlua::Error::RuntimeError(format!("Harness.{verb}: {msg}"))
}

/// The demo `world` replays, read again from its file.
fn on_demo(world: &Rc<RefCell<GameWorld>>, verb: &str) -> LuaResult<Demo> {
    let mut world = world.borrow_mut();
    let replayer = world
        .tap_mut::<Replayer>()
        .ok_or_else(|| fail(verb, NO_DEMO))?;
    Demo::read(replayer.demo_path()).map_err(|e| fail(verb, e))
}

pub(super) fn register(lua: &Lua, harness: &Shared, t: &Table) -> LuaResult<()> {
    // FindInDemo(query) -> { tick, ... }: the ticks whose index matches, in order.
    let world = world_of(harness);
    t.set(
        "FindInDemo",
        lua.create_function(move |_, query: Table| {
            let query = query_of(&query)?;
            Ok(on_demo(&world, "FindInDemo")?.find(&query))
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
            let demo = on_demo(&world, "CutDemo")?;
            demo::cut(demo.path(), demo.ticks, from, to, Path::new(&path))
                .map_err(|e| fail("CutDemo", e))
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
