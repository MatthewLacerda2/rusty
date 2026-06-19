//! src/api/animator.rs — `Animator` namespace.
//!
//! `Play`/`Crossfade`/`Stop` over an entity's optional animator component. `Play`
//! hard-cuts to a clip; `Crossfade` blends out of the current clip over a duration
//! (#80); both drive the component's minimal state machine, which the `animate`
//! system samples against the mesh's imported clips.

use std::cell::RefCell;

use mlua::Lua;

use super::{put, Reg};
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
                if let Some(anim) = &mut e.animator {
                    anim.is_playing = false;
                }
            }
            Ok(())
        }),
    )
}
