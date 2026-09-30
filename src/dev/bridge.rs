//! src/dev/bridge.rs — Lua bindings for the scenario VM.
//!
//! Registers the control surface a scenario script calls. This is a SEPARATE Lua VM
//! from the gameplay scripts inside `GameWorld` — the scenario drives the world from
//! the outside (Step/StepUntil), it does not run as an entity behaviour.
//!
//! Tables: Harness.{Step,StepUntil,Snapshot,Log,Expect,Frame,Stats,AssertBudget}, plus read helpers
//! Scene.FindEntityByName / Transform.GetPosition / Animator.GetClip and the
//! writable Input injection (Press/Release, MoveMouse, AddMouseDelta, Scroll,
//! TypeText). Shooting is just pressing the SPACE key the
//! player-controller script edge-detects — there is no separate click/shoot signal.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use mlua::{Function, Lua, Result as LuaResult};

use super::harness::{tick_unless_quit, Harness};
use crate::app::GameWorld;
use crate::core::input::InputState;

type Shared = Rc<RefCell<Harness>>;

fn world_of(h: &Shared) -> Rc<RefCell<GameWorld>> {
    Rc::clone(&h.borrow().world)
}

/// Register every scenario-facing table on `lua`'s globals.
pub fn register(lua: &Lua, harness: &Shared) -> LuaResult<()> {
    register_harness(lua, harness)?;
    register_scene(lua, harness)?;
    register_input(lua, harness)?;
    Ok(())
}

fn register_harness(lua: &Lua, harness: &Shared) -> LuaResult<()> {
    let t = lua.create_table()?;
    register_harness_stepping(lua, harness, &t)?;
    register_harness_reporting(lua, harness, &t)?;
    register_harness_actions(lua, harness, &t)?;
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
                if pred.call::<_, bool>(())? {
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
    // Screenshot(path) -> bool. Renders the current scene/camera offscreen to a
    // PNG. Returns false (no error) when no GPU/software adapter is available.
    let h = Rc::clone(harness);
    t.set(
        "Screenshot",
        lua.create_function(move |_, path: String| Ok(h.borrow_mut().screenshot(path)))?,
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

fn register_scene(lua: &Lua, harness: &Shared) -> LuaResult<()> {
    register_scene_lookup(lua, harness)?;
    register_scene_transform(lua, harness)?;
    register_scene_animator(lua, harness)
}

/// Scene.FindEntityByName — name -> entity id.
fn register_scene_lookup(lua: &Lua, harness: &Shared) -> LuaResult<()> {
    let scene_t = lua.create_table()?;
    let world = world_of(harness);
    scene_t.set(
        "FindEntityByName",
        lua.create_function(move |_, name: String| {
            Ok(world.borrow().scene().borrow().find_entity_by_name(&name))
        })?,
    )?;
    lua.globals().set("Scene", scene_t)
}

/// Transform.GetPosition — entity id -> world position tuple.
fn register_scene_transform(lua: &Lua, harness: &Shared) -> LuaResult<()> {
    let transform_t = lua.create_table()?;
    let world = world_of(harness);
    transform_t.set(
        "GetPosition",
        lua.create_function(move |_, id: u32| {
            let w = world.borrow();
            let s = w.scene().borrow();
            let p = s
                .world
                .transform(id)
                .map(|t| t.position)
                .unwrap_or(Vec3::ZERO);
            Ok((p.x, p.y, p.z))
        })?,
    )?;
    lua.globals().set("Transform", transform_t)
}

/// Animator.GetClip — entity id -> current clip name.
fn register_scene_animator(lua: &Lua, harness: &Shared) -> LuaResult<()> {
    let anim_t = lua.create_table()?;
    let world = world_of(harness);
    anim_t.set(
        "GetClip",
        lua.create_function(move |_, id: u32| {
            let w = world.borrow();
            let s = w.scene().borrow();
            let clip = s
                .world
                .animator(id)
                .map(|a| a.current_clip.clone())
                .unwrap_or_default();
            Ok(clip)
        })?,
    )?;
    lua.globals().set("Animator", anim_t)
}

/// The scenario's writable `Input`: the same injection verbs gameplay scripts get
/// (`api::input`), bound to the harness world. Injection bypasses the keymap.
fn register_input(lua: &Lua, harness: &Shared) -> LuaResult<()> {
    let t = lua.create_table()?;
    inject(lua, &t, harness, "Press", |i, key: String| i.press(&key))?;
    inject(lua, &t, harness, "Release", |i, key: String| {
        i.release(&key)
    })?;
    inject(lua, &t, harness, "MoveMouse", |i, (x, y)| {
        i.move_mouse(x, y)
    })?;
    inject(lua, &t, harness, "AddMouseDelta", |i, (dx, dy)| {
        i.add_mouse_delta(dx, dy)
    })?;
    inject(lua, &t, harness, "Scroll", |i, dy: f64| i.scroll(dy))?;
    inject(lua, &t, harness, "TypeText", |i, text: String| {
        i.type_text(&text)
    })?;
    lua.globals().set("Input", t)
}

/// Bind one injection verb: `f` writes the decoded Lua arguments into the world's input.
fn inject<A>(
    lua: &Lua,
    t: &mlua::Table,
    harness: &Shared,
    name: &str,
    f: impl Fn(&mut InputState, A) + 'static,
) -> LuaResult<()>
where
    A: for<'lua> mlua::FromLuaMulti<'lua>,
{
    let world = world_of(harness);
    t.set(
        name,
        lua.create_function(move |_, args: A| {
            f(&mut world.borrow().input().borrow_mut(), args);
            Ok(())
        })?,
    )
}
