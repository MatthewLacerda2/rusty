//! src/dev/bridge/ — the scenario-only half of the scenario VM's surface.
//!
//! A scenario runs in a SEPARATE Lua VM from the gameplay scripts inside
//! `GameWorld`: it drives the world from the outside (Step/StepUntil), it does not
//! run as an entity behaviour. Everything a gameplay script can call, a scenario
//! calls too: `scenario::run` registers the real API (`api::register`) over the
//! harness world's cells, read-write, the way a Unity editor test can call any
//! runtime API (#1384). That includes `Input`'s injection verbs (`Press`,
//! `MoveMouse`, `SetAxis`, …); shooting is just pressing `Mouse0`.
//!
//! This module adds only what is the scenario's own: the `Harness` table
//! (Step, StepUntil, Snapshot, Log, Expect, Frame, Stats, AssertBudget, Screenshot,
//! Render, AttachPlayerBot, LoadStress, Bench), `Scene.GetScript` answered across
//! the VM boundary (`script_ref`), and the refusal of what entering Play would wipe
//! (`before_play`).

mod before_play;
mod copy;
mod script_ref;
#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Function, Lua, Result as LuaResult};

use super::harness::{tick_unless_quit, Harness};
use crate::app::GameWorld;

type Shared = Rc<RefCell<Harness>>;

fn world_of(h: &Shared) -> Rc<RefCell<GameWorld>> {
    Rc::clone(&h.borrow().world)
}

/// Register the scenario-only surface on `lua`'s globals, after the API (it
/// replaces the API's `Scene.GetScript`, and guards what Play would wipe).
pub fn register(lua: &Lua, harness: &Shared) -> LuaResult<()> {
    register_harness(lua, harness)?;
    before_play::register(lua, &world_of(harness))?;
    script_ref::register(lua, &world_of(harness))
}

fn register_harness(lua: &Lua, harness: &Shared) -> LuaResult<()> {
    let t = lua.create_table()?;
    register_harness_stepping(lua, harness, &t)?;
    register_harness_reporting(lua, harness, &t)?;
    register_harness_actions(lua, harness, &t)?;
    super::bench::register(lua, harness, &t)?;
    lua.globals().set("Harness", t)
}

/// Step / StepUntil — advance the headless world by fixed-dt ticks.
fn register_harness_stepping(lua: &Lua, harness: &Shared, t: &mlua::Table) -> LuaResult<()> {
    let world = world_of(harness);
    t.set(
        "Step",
        lua.create_function(move |_, n: u32| {
            let mut w = world.borrow_mut();
            for _ in 0..n {
                if !tick_unless_quit(&mut w) {
                    break;
                }
            }
            Ok(())
        })?,
    )?;

    let world = world_of(harness);
    t.set(
        "StepUntil",
        lua.create_function(move |_, (pred, max): (Function, u32)| {
            for _ in 0..max {
                // A quit game ends the run: the predicate can no longer come true.
                if !tick_unless_quit(&mut world.borrow_mut()) {
                    return Ok(false);
                }
                if pred.call::<bool>(())? {
                    return Ok(true);
                }
            }
            Ok(false)
        })?,
    )
}

/// Snapshot / Log / Expect / Frame — observe and assert against the run.
fn register_harness_reporting(lua: &Lua, harness: &Shared, t: &mlua::Table) -> LuaResult<()> {
    // Snapshot is returned as a pretty JSON string — the agent reads it from
    // results.json anyway; in-scenario it is handy for logging a checkpoint.
    let h = Rc::clone(harness);
    t.set(
        "Snapshot",
        lua.create_function(move |_, ()| {
            let snap = h.borrow().snapshot();
            Ok(serde_json::to_string_pretty(&snap).unwrap_or_default())
        })?,
    )?;

    let h = Rc::clone(harness);
    t.set(
        "Log",
        lua.create_function(move |_, msg: String| {
            h.borrow_mut().log(msg);
            Ok(())
        })?,
    )?;

    let h = Rc::clone(harness);
    t.set(
        "Expect",
        lua.create_function(move |_, (cond, msg): (bool, String)| {
            h.borrow_mut().expect(cond, msg);
            Ok(())
        })?,
    )?;

    let h = Rc::clone(harness);
    t.set(
        "Frame",
        lua.create_function(move |_, ()| Ok(h.borrow().frame()))?,
    )?;
    register_harness_stats(lua, harness, t)
}

/// Stats / AssertBudget — the run's frame stats (#433) and budgets over them.
fn register_harness_stats(lua: &Lua, harness: &Shared, t: &mlua::Table) -> LuaResult<()> {
    let h = Rc::clone(harness);
    t.set(
        "Stats",
        lua.create_function(move |lua, ()| {
            let stats = Rc::clone(&h.borrow().stats);
            let stats = stats.borrow();
            super::stats::to_lua(lua, &stats)
        })?,
    )?;

    // AssertBudget{ metric = limit, ... } -> bool. Each budget becomes an
    // expectation: a metric's worst frame so far must not exceed its limit.
    let h = Rc::clone(harness);
    t.set(
        "AssertBudget",
        lua.create_function(move |_, budgets: mlua::Table| {
            let budgets = super::stats::budgets_from_lua(budgets)?;
            Ok(h.borrow_mut().assert_budget(&budgets))
        })?,
    )
}

/// Screenshot / AttachPlayerBot — side-effecting harness actions.
fn register_harness_actions(lua: &Lua, harness: &Shared, t: &mlua::Table) -> LuaResult<()> {
    // Screenshot(path [, { width =, height = }]) -> bool. Renders the current
    // scene/camera offscreen to a PNG, at the run's size or the one given (#1583).
    // Returns false (no error) when no GPU/software adapter is available.
    let h = Rc::clone(harness);
    t.set(
        "Screenshot",
        lua.create_function(move |_, (path, size): (String, Option<mlua::Table>)| {
            let mut h = h.borrow_mut();
            let size = match size {
                Some(size) => shot_size(&size)?,
                None => h.size(),
            };
            Ok(h.screenshot_at(path, size))
        })?,
    )?;

    // Render() -> bool. Draws the current frame offscreen, writing nothing, so its
    // cost (render counters, renderer_ms, GPU time) lands in Stats (#835).
    let h = Rc::clone(harness);
    t.set(
        "Render",
        lua.create_function(move |_, ()| Ok(h.borrow_mut().render_frame().is_some()))?,
    )?;

    // AttachPlayerBot(path) -> bool. Tag the Player with a bot-player script so the
    // headless world runs it from Update() on the first Step. Call this BEFORE any
    // Step — scripts load when the world enters play mode (first tick). Returns true
    // if a Player entity was found and tagged.
    let world = world_of(harness);
    t.set(
        "AttachPlayerBot",
        lua.create_function(move |_, path: String| {
            let w = world.borrow();
            let attached = super::botplayer::attach_player_bot(&mut w.scene().borrow_mut(), &path);
            Ok(attached)
        })?,
    )
}

/// A `Screenshot`'s `{ width =, height = }`: both whole pixels, 1 to
/// `video::MAX_SIDE`, as `play --size` takes them.
fn shot_size(size: &mlua::Table) -> LuaResult<(u32, u32)> {
    use crate::core::settings::video::MAX_SIDE;
    let side = |key: &str| -> LuaResult<u32> {
        let value: Option<i64> = size.get(key)?;
        value
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| (1..=MAX_SIDE).contains(n))
            .ok_or_else(|| {
                let msg = format!("Harness.Screenshot: {key} must be a whole 1..={MAX_SIDE}");
                mlua::Error::RuntimeError(msg)
            })
    };
    Ok((side("width")?, side("height")?))
}
