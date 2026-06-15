//! src/api/animator.rs — `Animator` namespace.
//!
//! `Play`/`Crossfade`/`Stop` over an entity's optional animator component.
//! `Crossfade` is simplified to a plain play in our component model. Scoped
//! callbacks over the borrowed scene + console (#57).

use std::cell::RefCell;

use mlua::{Lua, Scope};

use super::{put, Reg};
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// Register the `Animator` namespace onto `lua`.
pub fn register<'a, 'scope>(
    scope: &Scope<'a, 'scope>,
    lua: &'a Lua,
    scene: &'scope RefCell<&'scope mut Scene>,
    console: &'scope RefCell<&'scope mut ConsoleLogs>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "Play",
        scope.create_function(move |_, (id, clip): (u32, String)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(anim) = &mut e.animator {
                    anim.current_clip = clip.clone();
                    anim.is_playing = true;
                    anim.freeze = false;
                    console
                        .borrow_mut()
                        .info(format!("Entity {} playing animation: {}", e.name, clip));
                }
            }
            Ok(())
        }),
    )?;

    put(
        &table,
        "Crossfade",
        scope.create_function(move |_, (id, clip, _duration): (u32, String, f32)| {
            // Simplify crossfade to standard play in our component
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(anim) = &mut e.animator {
                    anim.current_clip = clip;
                    anim.is_playing = true;
                    anim.freeze = false;
                }
            }
            Ok(())
        }),
    )?;

    put(
        &table,
        "Stop",
        scope.create_function(move |_, id: u32| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(anim) = &mut e.animator {
                    anim.is_playing = false;
                }
            }
            Ok(())
        }),
    )?;

    lua.globals()
        .set("Animator", table)
        .map_err(|e| e.to_string())
}
