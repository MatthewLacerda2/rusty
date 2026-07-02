//! src/api/animator.rs — `Animator` namespace.
//!
//! `Play`/`Crossfade`/`Stop` plus `Pause`/`Resume` and `SetLooping` (#313) over an
//! entity's optional animator component. `Play` hard-cuts to a clip; `Crossfade`
//! blends out of the current clip over a duration (#80); both drive the component's
//! minimal state machine, which the `animate` system samples against the mesh's
//! imported clips. `Pause`/`Resume` hold and release the playhead over the
//! component's `freeze` flag; `SetLooping` wraps the playhead at the clip's end.

use std::cell::RefCell;

use mlua::Lua;

use super::{put, Reg};
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
            if let Some(mut e) = scene.get_entity_mut(id) {
                animator_ops::set_playing(&mut e, false);
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
            if let Some(mut e) = scene.get_entity_mut(id) {
                animator_ops::set_freeze(&mut e, true);
            }
            Ok(())
        }),
    )?;

    put(
        table,
        "Resume",
        scope.create_function(|_, id: u32| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                animator_ops::set_freeze(&mut e, false);
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
            if let Some(mut e) = scene.get_entity_mut(id) {
                animator_ops::set_looping(&mut e, looping);
            }
            Ok(())
        }),
    )
}
