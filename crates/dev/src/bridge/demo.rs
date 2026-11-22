use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use glam::Vec3;
use mlua::{Lua, Result as LuaResult, Table, Value};

use super::{world_of, Shared};
use crate::demo::{self, EventKind, Query, Replayer, NO_DEMO};
use rusty_app::GameWorld;

fn fail(verb: &str, msg: impl std::fmt::Display) -> mlua::Error {
    mlua::Error::RuntimeError(format!("Harness.{verb}: {msg}"))
}

fn on_demo(world: &Rc<RefCell<GameWorld>>, verb: &str) -> LuaResult<(PathBuf, u64)> {
    let mut world = world.borrow_mut();
    let replayer = world
        .tap_mut::<Replayer>()
        .ok_or_else(|| fail(verb, NO_DEMO))?;
    Ok((replayer.demo_path().to_path_buf(), replayer.verdict().of))
}

pub(super) fn register(lua: &Lua, harness: &Shared, t: &Table) -> LuaResult<()> {
    let world = world_of(harness);
    t.set(
        "FindInDemo",
        lua.create_function(move |_, query: Table| {
            let query = query_of(&query)?;
            let (path, ticks) = on_demo(&world, "FindInDemo")?;
            Ok(demo::find_in(&path, ticks, &query))
        })?,
    )?;

    let world = world_of(harness);
    t.set(
        "SeekDemo",
        lua.create_function(move |_, tick: u64| {
            demo::seek(&mut world.borrow_mut(), tick).map_err(|e| fail("SeekDemo", e))
        })?,
    )?;

    let world = world_of(harness);
    t.set(
        "CutDemo",
        lua.create_function(move |_, (from, to, path): (u64, u64, String)| {
            let (demo, ticks) = on_demo(&world, "CutDemo")?;
            demo::cut(&demo, ticks, from, to, Path::new(&path)).map_err(|e| fail("CutDemo", e))
        })?,
    )?;

    let world = world_of(harness);
    t.set(
        "DemoPath",
        lua.create_function(move |_, ()| {
            let path = on_demo(&world, "DemoPath").ok().map(|(path, _)| path);
            Ok(path.map(|p| demo::full_path(&p).to_string_lossy().into_owned()))
        })?,
    )
}

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
