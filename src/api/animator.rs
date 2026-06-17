//! src/api/animator.rs — `Animator` namespace.
//!
//! `Play`/`Crossfade`/`Stop` over an entity's optional animator component. `Play`
//! hard-cuts to a clip; `Crossfade` blends out of the current clip over a duration
//! (#80); both drive the component's minimal state machine, which the `animate`
//! system samples against the mesh's imported clips.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;

use super::{put, Reg};
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// Register the `Animator` namespace onto `lua`.
pub fn register(lua: &Lua, scene: &Rc<RefCell<Scene>>, console: &Rc<RefCell<ConsoleLogs>>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_play(lua, &table, scene, console)?;
    register_stop(lua, &table, scene)?;

    lua.globals()
        .set("Animator", table)
        .map_err(|e| e.to_string())
}

/// `Play` / `Crossfade` — start a clip (crossfade simplifies to a plain play).
fn register_play(
    lua: &Lua,
    table: &mlua::Table,
    scene: &Rc<RefCell<Scene>>,
    console: &Rc<RefCell<ConsoleLogs>>,
) -> Reg {
    let s = Rc::clone(scene);
    let c = Rc::clone(console);
    put(
        table,
        "Play",
        lua.create_function(move |_, (id, clip): (u32, String)| {
            let mut scene = s.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                if let Some(anim) = &mut e.animator {
                    anim.play(clip.clone());
                    c.borrow_mut()
                        .info(format!("Entity {} playing animation: {}", e.name, clip));
                }
            }
            Ok(())
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        table,
        "Crossfade",
        lua.create_function(move |_, (id, clip, duration): (u32, String, f32)| {
            // Blend out of the current clip over `duration` seconds (#80).
            let mut scene = s.borrow_mut();
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
fn register_stop(lua: &Lua, table: &mlua::Table, scene: &Rc<RefCell<Scene>>) -> Reg {
    let s = Rc::clone(scene);
    put(
        table,
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
    )
}
