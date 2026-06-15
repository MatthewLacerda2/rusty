//! src/api/animator.rs — `Animator` namespace.
//!
//! `Play`/`Crossfade`/`Stop` over an entity's optional animator component.
//! `Crossfade` is simplified to a plain play in our component model.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;

use super::{put, Reg};
use crate::core::scene::Scene;
use crate::scripting::ConsoleLogs;

/// Register the `Animator` namespace onto `lua`.
pub fn register(lua: &Lua, scene: &Rc<RefCell<Scene>>, console: &Rc<RefCell<ConsoleLogs>>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    let s = Rc::clone(scene);
    let c = Rc::clone(console);
    put(
        &table,
        "Play",
        lua.create_function(move |_, (id, clip): (u32, String)| {
            let mut scene = s.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(anim) = &mut e.animator {
                    anim.current_clip = clip.clone();
                    anim.is_playing = true;
                    anim.freeze = false;
                    c.borrow_mut()
                        .info(format!("Entity {} playing animation: {}", e.name, clip));
                }
            }
            Ok(())
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "Crossfade",
        lua.create_function(move |_, (id, clip, _duration): (u32, String, f32)| {
            // Simplify crossfade to standard play in our component
            let mut scene = s.borrow_mut();
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

    let s = Rc::clone(scene);
    put(
        &table,
        "Stop",
        lua.create_function(move |_, id: u32| {
            let mut scene = s.borrow_mut();
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
