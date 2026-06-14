//! src/dev/bridge.rs — Lua bindings for the scenario VM.
//!
//! Registers the control surface a scenario script calls. This is a SEPARATE Lua VM
//! from the gameplay scripts inside `GameWorld` — the scenario drives the world from
//! the outside (Step/StepUntil), it does not run as an entity behaviour.
//!
//! Tables: Harness.{Step,StepUntil,Snapshot,Log,Expect,Frame}, plus read helpers
//! Scene.FindEntityByName / Transform.GetPosition / Health.Get / Animator.GetClip and
//! the writable Input.{Press,Release,Click}.

use std::cell::RefCell;
use std::rc::Rc;

use glam::Vec3;
use mlua::{Function, Lua, Result as LuaResult};

use super::harness::{Harness, FIXED_DT};
use crate::app::GameWorld;

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

    let world = world_of(harness);
    t.set(
        "Step",
        lua.create_function(move |_, n: u32| {
            let mut w = world.borrow_mut();
            for _ in 0..n {
                w.tick(FIXED_DT);
            }
            Ok(())
        })?,
    )?;

    let world = world_of(harness);
    t.set(
        "StepUntil",
        lua.create_function(move |_, (pred, max): (Function, u32)| {
            for _ in 0..max {
                {
                    let mut w = world.borrow_mut();
                    w.tick(FIXED_DT);
                }
                if pred.call::<_, bool>(())? {
                    return Ok(true);
                }
            }
            Ok(false)
        })?,
    )?;

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

    // Screenshot(path) -> bool. Renders the current scene/camera offscreen to a
    // PNG. Returns false (no error) when no GPU/software adapter is available.
    let h = Rc::clone(harness);
    t.set(
        "Screenshot",
        lua.create_function(move |_, path: String| Ok(h.borrow_mut().screenshot(path)))?,
    )?;

    lua.globals().set("Harness", t)
}

fn register_scene(lua: &Lua, harness: &Shared) -> LuaResult<()> {
    let scene_t = lua.create_table()?;
    let world = world_of(harness);
    scene_t.set(
        "FindEntityByName",
        lua.create_function(move |_, name: String| {
            Ok(world.borrow().scene.borrow().find_entity_by_name(&name))
        })?,
    )?;
    lua.globals().set("Scene", scene_t)?;

    let transform_t = lua.create_table()?;
    let world = world_of(harness);
    transform_t.set(
        "GetPosition",
        lua.create_function(move |_, id: u32| {
            let w = world.borrow();
            let s = w.scene.borrow();
            let p = s
                .get_entity(id)
                .map(|e| e.transform.position)
                .unwrap_or(Vec3::ZERO);
            Ok((p.x, p.y, p.z))
        })?,
    )?;
    lua.globals().set("Transform", transform_t)?;

    let health_t = lua.create_table()?;
    let world = world_of(harness);
    health_t.set(
        "Get",
        lua.create_function(move |_, id: u32| {
            let w = world.borrow();
            let s = w.scene.borrow();
            let hp = s
                .get_entity(id)
                .and_then(|e| e.health.as_ref().map(|h| h.current_health))
                .unwrap_or(0.0);
            Ok(hp)
        })?,
    )?;
    lua.globals().set("Health", health_t)?;

    let anim_t = lua.create_table()?;
    let world = world_of(harness);
    anim_t.set(
        "GetClip",
        lua.create_function(move |_, id: u32| {
            let w = world.borrow();
            let s = w.scene.borrow();
            let clip = s
                .get_entity(id)
                .and_then(|e| e.animator.as_ref().map(|a| a.current_clip.clone()))
                .unwrap_or_default();
            Ok(clip)
        })?,
    )?;
    lua.globals().set("Animator", anim_t)
}

fn register_input(lua: &Lua, harness: &Shared) -> LuaResult<()> {
    let t = lua.create_table()?;

    let world = world_of(harness);
    t.set(
        "Press",
        lua.create_function(move |_, key: String| {
            world.borrow().input.borrow_mut().press(&key);
            Ok(())
        })?,
    )?;

    let world = world_of(harness);
    t.set(
        "Release",
        lua.create_function(move |_, key: String| {
            world.borrow().input.borrow_mut().release(&key);
            Ok(())
        })?,
    )?;

    let world = world_of(harness);
    t.set(
        "Click",
        lua.create_function(move |_, ()| {
            world.borrow().input.borrow_mut().mouse_left_clicked = true;
            Ok(())
        })?,
    )?;

    lua.globals().set("Input", t)
}
