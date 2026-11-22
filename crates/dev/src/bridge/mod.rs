mod before_play;
mod copy;
mod demo;
mod forward;
mod script_ref;
#[cfg(test)]
mod tests;
mod warnings;

use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Function, Lua, Result as LuaResult};

use super::harness::{tick_unless_quit, Harness};
use rusty_app::GameWorld;

type Shared = Rc<RefCell<Harness>>;

fn world_of(h: &Shared) -> Rc<RefCell<GameWorld>> {
    Rc::clone(&h.borrow().world)
}

pub fn register(lua: &Lua, harness: &Shared) -> LuaResult<()> {
    register_harness(lua, harness)?;
    before_play::register(lua, &world_of(harness))?;
    forward::register(lua, &world_of(harness))?;
    script_ref::register(lua, &world_of(harness))
}

fn register_harness(lua: &Lua, harness: &Shared) -> LuaResult<()> {
    let t = lua.create_table()?;
    register_harness_stepping(lua, harness, &t)?;
    register_harness_reporting(lua, harness, &t)?;
    warnings::register(lua, harness, &t)?;
    register_harness_actions(lua, harness, &t)?;
    super::bench::register(lua, harness, &t)?;
    super::harness::audio::register(lua, harness, &t)?;
    demo::register(lua, harness, &t)?;
    lua.globals().set("Harness", t)
}

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

fn register_harness_reporting(lua: &Lua, harness: &Shared, t: &mlua::Table) -> LuaResult<()> {
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

    let h = Rc::clone(harness);
    t.set(
        "AssertBudget",
        lua.create_function(move |_, budgets: mlua::Table| {
            let budgets = super::stats::budgets_from_lua(budgets)?;
            Ok(h.borrow_mut().assert_budget(&budgets))
        })?,
    )
}

fn register_harness_actions(lua: &Lua, harness: &Shared, t: &mlua::Table) -> LuaResult<()> {
    let h = Rc::clone(harness);
    t.set(
        "Screenshot",
        lua.create_function(move |_, (path, opts): (String, Option<mlua::Table>)| {
            let mut h = h.borrow_mut();
            let shot = match opts {
                Some(opts) => shot_options(&opts)?,
                None => ShotOptions::default(),
            };
            let size = shot.size.unwrap_or_else(|| h.size());
            Ok(h.screenshot_settled(path, size, (shot.gizmos, shot.settle)))
        })?,
    )?;

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

/// Most frames a `Screenshot` may settle: TAA's default blend keeps 0.9^64, about 0.1%, of
/// an old view after 64 frames, so 1024 only catches a typo.
const MAX_SETTLE: u32 = 1024;

#[derive(Default)]
struct ShotOptions {
    size: Option<(u32, u32)>,
    gizmos: bool,
    settle: u32,
}

fn shot_options(opts: &mlua::Table) -> LuaResult<ShotOptions> {
    let gizmos = match opts.get::<mlua::Value>("gizmos")? {
        mlua::Value::Nil => false,
        mlua::Value::Boolean(on) => on,
        _ => {
            let msg = "Harness.Screenshot: gizmos must be a boolean";
            return Err(mlua::Error::RuntimeError(msg.into()));
        }
    };
    let sized = opts.contains_key("width")? || opts.contains_key("height")?;
    let size = if sized { Some(shot_size(opts)?) } else { None };
    let settle: Option<i64> = opts.get("settle")?;
    let settle = match settle.map(|n| u32::try_from(n).ok().filter(|n| *n <= MAX_SETTLE)) {
        None => 0,
        Some(Some(n)) => n,
        Some(None) => {
            let msg = format!("Harness.Screenshot: settle must be a whole 0..={MAX_SETTLE}");
            return Err(mlua::Error::RuntimeError(msg));
        }
    };
    Ok(ShotOptions {
        size,
        gizmos,
        settle,
    })
}

fn shot_size(size: &mlua::Table) -> LuaResult<(u32, u32)> {
    use rusty_core::settings::video::MAX_SIDE;
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
