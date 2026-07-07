//! src/api/animator/ — `Animator` namespace.
//!
//! `Play`/`Crossfade`/`Stop` plus `Pause`/`Resume` and `SetLooping` (#313) over an
//! entity's optional animator component. `Play` hard-cuts to a clip; `Crossfade`
//! blends out of the current clip over a duration (#80); both drive the component's
//! minimal state machine, which the `animate` system samples against the mesh's
//! imported clips. `Pause`/`Resume` hold and release the playhead over the
//! component's `freeze` flag; `SetLooping` wraps the playhead at the clip's end.
//! `SetBool`/`SetFloat`/`SetInt`/`SetTrigger` (#314) write the typed graph
//! parameters the animation-graph evaluator (#316) reads to decide transitions.
//! The graph control surface (#316: `SetGraph`, `SetGraphEnabled`, `PlayNode`,
//! `PlayAnimation`, `GetCurrentNode`) lives in `graph.rs` — one namespace, split
//! to stay under the size cap.

mod graph;

use std::cell::RefCell;

use mlua::Lua;

use super::{put, Reg};
use crate::components::AnimatorParameter;
use crate::scene::authoring::animator as animator_ops;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// Register the `Animator` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
    console: &'scope RefCell<ConsoleLogs>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_play(scope, &table, scene, console)?;
    register_stop(scope, &table, scene)?;
    register_pause_resume(scope, &table, scene)?;
    register_set_looping(scope, &table, scene)?;
    register_set_bool_float(scope, &table, scene, console)?;
    register_set_int_trigger(scope, &table, scene, console)?;
    graph::register(scope, &table, scene, console)?;

    lua.globals()
        .set("Animator", table)
        .map_err(|e| e.to_string())
}

/// `Play` / `Crossfade` — start a clip (crossfade simplifies to a plain play).
fn register_play<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    console: &'scope RefCell<ConsoleLogs>,
) -> Reg {
    put(
        table,
        "Play",
        scope.create_function(|_, (id, clip): (u32, String)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(anim) = &mut e.animator {
                    anim.play(clip.clone());
                    console
                        .borrow_mut()
                        .info(format!("Entity {} playing animation: {}", e.name, clip));
                }
            }
            Ok(())
        }),
    )?;

    put(
        table,
        "Crossfade",
        scope.create_function(|_, (id, clip, duration): (u32, String, f32)| {
            // Blend out of the current clip over `duration` seconds (#80).
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(anim) = &mut e.animator {
                    anim.crossfade(clip, duration);
                }
            }
            Ok(())
        }),
    )
}

/// `Stop` — halt playback on the entity's animator.
fn register_stop<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "Stop",
        scope.create_function(|_, id: u32| {
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.animator_mut(id) {
                animator_ops::set_playing(&mut c, false);
            }
            Ok(())
        }),
    )
}

/// `Pause` / `Resume` — hold and release the playhead over the component's `freeze`
/// flag (#313). Unlike `Stop`, `is_playing` stays set: the pose holds where it is
/// and `Resume` continues from the same frame.
fn register_pause_resume<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "Pause",
        scope.create_function(|_, id: u32| {
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.animator_mut(id) {
                animator_ops::set_freeze(&mut c, true);
            }
            Ok(())
        }),
    )?;

    put(
        table,
        "Resume",
        scope.create_function(|_, id: u32| {
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.animator_mut(id) {
                animator_ops::set_freeze(&mut c, false);
            }
            Ok(())
        }),
    )
}

/// `SetLooping` — wrap the playhead at the current clip's end so it repeats
/// seamlessly (idle/run/walk cycles, #313). Off, the last frame holds as before.
fn register_set_looping<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetLooping",
        scope.create_function(|_, (id, looping): (u32, bool)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.animator_mut(id) {
                animator_ops::set_looping(&mut c, looping);
            }
            Ok(())
        }),
    )
}

/// `SetBool` / `SetFloat` — write a typed graph parameter (#314).
fn register_set_bool_float<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    console: &'scope RefCell<ConsoleLogs>,
) -> Reg {
    put(
        table,
        "SetBool",
        scope.create_function(|_, (id, name, value): (u32, String, bool)| {
            write_parameter(scene, console, id, &name, AnimatorParameter::Bool(value));
            Ok(())
        }),
    )?;

    put(
        table,
        "SetFloat",
        scope.create_function(|_, (id, name, value): (u32, String, f32)| {
            write_parameter(scene, console, id, &name, AnimatorParameter::Float(value));
            Ok(())
        }),
    )
}

/// `SetInt` / `SetTrigger` — the remaining parameter setters (#314). `SetTrigger`
/// takes no value: it latches the one-shot true, and the graph evaluator (#316)
/// auto-clears it when it consumes a transition.
fn register_set_int_trigger<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    console: &'scope RefCell<ConsoleLogs>,
) -> Reg {
    put(
        table,
        "SetInt",
        scope.create_function(|_, (id, name, value): (u32, String, i32)| {
            write_parameter(scene, console, id, &name, AnimatorParameter::Int(value));
            Ok(())
        }),
    )?;

    put(
        table,
        "SetTrigger",
        scope.create_function(|_, (id, name): (u32, String)| {
            write_parameter(scene, console, id, &name, AnimatorParameter::Trigger(true));
            Ok(())
        }),
    )
}

/// Write one typed parameter on the entity's animator. A name already bound to a
/// DIFFERENT type is a warned no-op, never a panic (#314); a missing entity or
/// animator is silently skipped, like every other `Animator.*` binding.
fn write_parameter(
    scene: &RefCell<Scene>,
    console: &RefCell<ConsoleLogs>,
    id: u32,
    name: &str,
    value: AnimatorParameter,
) {
    let mut scene = scene.borrow_mut();
    if let Some(mut e) = scene.get_entity_mut(id) {
        if let Some(anim) = &mut e.animator {
            if !anim.set_parameter(name, value) {
                console.borrow_mut().warn(format!(
                    "Animator parameter '{}' on entity {} already holds a different type; set ignored",
                    name, e.name
                ));
            }
        }
    };
}
