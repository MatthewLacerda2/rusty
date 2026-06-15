//! src/dev/bridge.rs — Lua bindings for the scenario VM.
//!
//! Registers the control surface a scenario script calls. This is a SEPARATE Lua VM
//! from the gameplay scripts inside `GameWorld` — the scenario drives the world from
//! the outside (Step/StepUntil), it does not run as an entity behaviour.
//!
//! Tables: Harness.{Step,StepUntil,Snapshot,Log,Expect,Frame}, plus read helpers
//! Scene.FindEntityByName / Transform.GetPosition / Health.Get / Animator.GetClip and
//! the writable Input.{Press,Release}. Shooting is just pressing the SPACE key the
//! player-controller script edge-detects — there is no separate click/shoot signal.
//!
//! The scenario VM drives the harness through a transient `RefCell<&mut Harness>`
//! opened for the scenario's run (#57): the bindings are scoped callbacks that
//! `borrow_mut()` it transiently and drop the borrow before re-entering Lua (e.g.
//! `StepUntil`'s predicate call). No `Rc`, no shared interior mutability.

use std::cell::RefCell;

use glam::Vec3;
use mlua::{Function, Lua, Result as LuaResult, Scope};

use super::harness::Harness;

/// The borrowed harness the scenario bindings drive, shared for the run's scope.
type Cell<'a> = RefCell<&'a mut Harness>;

/// Register every scenario-facing table on `lua`'s globals via scoped callbacks
/// over the borrowed `harness` cell.
pub fn register<'a, 'scope>(
    scope: &Scope<'a, 'scope>,
    lua: &'a Lua,
    harness: &'scope Cell<'scope>,
) -> LuaResult<()> {
    register_harness(scope, lua, harness)?;
    register_scene(scope, lua, harness)?;
    register_input(scope, lua, harness)?;
    Ok(())
}

fn register_harness<'a, 'scope>(
    scope: &Scope<'a, 'scope>,
    lua: &'a Lua,
    harness: &'scope Cell<'scope>,
) -> LuaResult<()> {
    let t = lua.create_table()?;

    t.set(
        "Step",
        scope.create_function(move |_, n: u32| {
            harness.borrow_mut().step(n);
            Ok(())
        })?,
    )?;

    t.set(
        "StepUntil",
        scope.create_function(move |_, (pred, max): (Function, u32)| {
            for _ in 0..max {
                // Drop the harness borrow before calling back into Lua: the
                // predicate may itself touch the scenario surface re-entrantly.
                harness.borrow_mut().step(1);
                if pred.call::<_, bool>(())? {
                    return Ok(true);
                }
            }
            Ok(false)
        })?,
    )?;

    // Snapshot is returned as a pretty JSON string — the agent reads it from
    // results.json anyway; in-scenario it is handy for logging a checkpoint.
    t.set(
        "Snapshot",
        scope.create_function(move |_, ()| {
            let snap = harness.borrow().snapshot();
            Ok(serde_json::to_string_pretty(&snap).unwrap_or_default())
        })?,
    )?;

    t.set(
        "Log",
        scope.create_function(move |_, msg: String| {
            harness.borrow_mut().log(msg);
            Ok(())
        })?,
    )?;

    t.set(
        "Expect",
        scope.create_function(move |_, (cond, msg): (bool, String)| {
            harness.borrow_mut().expect(cond, msg);
            Ok(())
        })?,
    )?;

    t.set(
        "Frame",
        scope.create_function(move |_, ()| Ok(harness.borrow().frame()))?,
    )?;

    // Screenshot(path) -> bool. Renders the current scene/camera offscreen to a
    // PNG. Returns false (no error) when no GPU/software adapter is available.
    t.set(
        "Screenshot",
        scope.create_function(move |_, path: String| Ok(harness.borrow_mut().screenshot(path)))?,
    )?;

    // AttachPlayerBot(path) -> bool. Tag the Player with a bot-player script so the
    // headless world runs it from Update() on the first Step. Call this BEFORE any
    // Step — scripts load when the world enters play mode (first tick). Returns true
    // if a Player entity was found and tagged.
    t.set(
        "AttachPlayerBot",
        scope.create_function(move |_, path: String| {
            let mut h = harness.borrow_mut();
            let attached = super::botplayer::attach_player_bot(h.world.scene_mut(), &path);
            Ok(attached)
        })?,
    )?;

    lua.globals().set("Harness", t)
}

fn register_scene<'a, 'scope>(
    scope: &Scope<'a, 'scope>,
    lua: &'a Lua,
    harness: &'scope Cell<'scope>,
) -> LuaResult<()> {
    let scene_t = lua.create_table()?;
    scene_t.set(
        "FindEntityByName",
        scope.create_function(move |_, name: String| {
            Ok(harness.borrow().world.scene().find_entity_by_name(&name))
        })?,
    )?;
    lua.globals().set("Scene", scene_t)?;

    let transform_t = lua.create_table()?;
    transform_t.set(
        "GetPosition",
        scope.create_function(move |_, id: u32| {
            let h = harness.borrow();
            let p = h
                .world
                .scene()
                .get_entity(id)
                .map(|e| e.transform.position)
                .unwrap_or(Vec3::ZERO);
            Ok((p.x, p.y, p.z))
        })?,
    )?;
    lua.globals().set("Transform", transform_t)?;

    let health_t = lua.create_table()?;
    health_t.set(
        "Get",
        scope.create_function(move |_, id: u32| {
            let h = harness.borrow();
            let hp = h
                .world
                .scene()
                .get_entity(id)
                .and_then(|e| e.health.as_ref().map(|h| h.current_health))
                .unwrap_or(0.0);
            Ok(hp)
        })?,
    )?;
    lua.globals().set("Health", health_t)?;

    let anim_t = lua.create_table()?;
    anim_t.set(
        "GetClip",
        scope.create_function(move |_, id: u32| {
            let h = harness.borrow();
            let clip = h
                .world
                .scene()
                .get_entity(id)
                .and_then(|e| e.animator.as_ref().map(|a| a.current_clip.clone()))
                .unwrap_or_default();
            Ok(clip)
        })?,
    )?;
    lua.globals().set("Animator", anim_t)
}

fn register_input<'a, 'scope>(
    scope: &Scope<'a, 'scope>,
    lua: &'a Lua,
    harness: &'scope Cell<'scope>,
) -> LuaResult<()> {
    let t = lua.create_table()?;

    t.set(
        "Press",
        scope.create_function(move |_, key: String| {
            harness.borrow_mut().world.input_mut().press(&key);
            Ok(())
        })?,
    )?;

    t.set(
        "Release",
        scope.create_function(move |_, key: String| {
            harness.borrow_mut().world.input_mut().release(&key);
            Ok(())
        })?,
    )?;

    lua.globals().set("Input", t)
}
