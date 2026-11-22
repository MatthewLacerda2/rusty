use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Lua, Result as LuaResult, Table};

use super::stress::{self, StressSpec};
use super::window::Run;
use super::Report;
use crate::harness::Harness;

pub fn register(lua: &Lua, harness: &Rc<RefCell<Harness>>, t: &Table) -> LuaResult<()> {
    let h = Rc::clone(harness);
    t.set(
        "LoadStress",
        lua.create_function(move |lua, opts: Option<Table>| {
            let spec = spec_from_lua(lua, opts)?;
            let h = h.borrow();
            let world = h.world.borrow();
            let spawned = stress::load(&mut world.scene().borrow_mut(), &spec);
            let out = lua.create_table()?;
            out.set("enemies", spawned.enemies)?;
            out.set("lights", spawned.lights)?;
            out.set("decals", spawned.decals)?;
            out.set("particle_systems", spawned.particle_systems)?;
            Ok(out)
        })?,
    )?;

    let h = Rc::clone(harness);
    let run = Rc::new(RefCell::new(Run::default()));
    t.set(
        "Bench",
        lua.create_function(move |lua, (frames, window): (u32, Option<String>)| {
            let mut harness = h.borrow_mut();
            let mut run = run.borrow_mut();
            let report = match window {
                None => run.first(&mut harness, frames),
                Some(name) => {
                    (run.then(&mut harness, frames, &name)).map_err(mlua::Error::runtime)?
                }
            };
            report_to_lua(lua, &report)
        })?,
    )
}

pub(crate) fn spec_from_lua(lua: &Lua, opts: Option<Table>) -> LuaResult<StressSpec> {
    let mut spec = StressSpec::default();
    let Some(opts) = opts else {
        return Ok(spec);
    };
    for pair in opts.pairs::<String, mlua::Value>() {
        let (key, value) = pair?;
        let int = || -> LuaResult<u32> { lua.unpack(value.clone()) };
        match key.as_str() {
            "enemies" => spec.enemies = int()?,
            "lights" => spec.lights = int()?,
            "flickering" => spec.flickering = int()?,
            "shadowed" => spec.shadowed = int()?,
            "decals" => spec.decals = int()?,
            "particle_systems" => spec.particle_systems = int()?,
            "enemy_script" => spec.enemy_script = string(&value)?,
            "flicker_script" => spec.flicker_script = string(&value)?,
            _ => {
                return Err(mlua::Error::runtime(format!(
                    "LoadStress: unknown option `{key}`"
                )))
            }
        }
    }
    Ok(spec)
}

fn string(value: &mlua::Value) -> LuaResult<String> {
    match value {
        mlua::Value::String(s) => Ok(s.to_str()?.to_string()),
        other => Err(mlua::Error::runtime(format!(
            "LoadStress: expected a path string, got {}",
            other.type_name()
        ))),
    }
}

fn report_to_lua(lua: &Lua, report: &Report) -> LuaResult<Table> {
    let metrics = lua.create_table()?;
    for (name, s) in &report.metrics {
        let row = lua.create_table()?;
        row.set("avg", s.avg)?;
        row.set("p95", s.p95)?;
        metrics.set(name.as_str(), row)?;
    }
    let out = lua.create_table()?;
    out.set("frames", report.frames)?;
    out.set("metrics", metrics)?;
    out.set(
        "warnings",
        lua.create_sequence_from(report.warnings.iter().cloned())?,
    )?;
    Ok(out)
}

#[cfg(test)]
#[path = "lua_tests.rs"]
mod lua_tests;
