//! The scenario-VM verbs behind `make bench`: `Harness.LoadStress([opts])` and
//! `Harness.Bench(frames)`. Registered onto the `Harness` table by the bridge.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Lua, Result as LuaResult, Table};

use super::stress::{self, StressSpec};
use super::Report;
use crate::dev::harness::Harness;

/// Add `LoadStress` and `Bench` to the scenario's `Harness` table `t`.
pub fn register(lua: &Lua, harness: &Rc<RefCell<Harness>>, t: &Table) -> LuaResult<()> {
    // LoadStress([opts]) -> { enemies, lights, decals, particle_systems }. Spawns
    // the stress scene onto the yard; call it before the first Step, so the
    // soldiers' and lights' scripts load when play starts.
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

    // Bench(frames) -> { frames, metrics = { <name> = { avg, p95 } } }. Steps and
    // renders `frames` frames, prints the report and keeps it in the out dir.
    let h = Rc::clone(harness);
    t.set(
        "Bench",
        lua.create_function(move |lua, frames: u32| {
            let report = super::run(&mut h.borrow_mut(), frames);
            report_to_lua(lua, &report)
        })?,
    )
}

/// A `LoadStress` options table over [`StressSpec::default`]. An unknown key is an
/// error, so a typo cannot silently bench the wrong scene.
fn spec_from_lua(lua: &Lua, opts: Option<Table>) -> LuaResult<StressSpec> {
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
    Ok(out)
}
