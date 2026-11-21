use std::cell::RefCell;

use mlua::Lua;

use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::scripting::{ConsoleLogs, LogLevel};
use rusty_scene::Scene;

use super::{put, Reg};

pub(super) fn register(
    lua: &Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    console: ApiCell<ConsoleLogs>,
) -> Reg {
    put(
        table,
        "Play",
        lua.create_function(move |lua, (id, clip): (EntityArg, String)| {
            let id = id.live("Animator", "Play")?;
            let (scene, console) = (scene.get(lua)?, console.get(lua)?);
            let mut scene = scene.borrow_mut();
            warn_unknown_clip(&scene, id, &clip, "Play", &console);
            if let Some(mut anim) = scene.world.animator_mut(id) {
                anim.play(clip);
            }
            Ok(())
        }),
    )?;

    put(
        table,
        "Crossfade",
        lua.create_function(move |lua, (id, clip, duration): (EntityArg, String, f32)| {
            let id = id.live("Animator", "Crossfade")?;
            let (scene, console) = (scene.get(lua)?, console.get(lua)?);
            let mut scene = scene.borrow_mut();
            warn_unknown_clip(&scene, id, &clip, "Crossfade", &console);
            if let Some(mut anim) = scene.world.animator_mut(id) {
                anim.crossfade(clip, duration);
            }
            Ok(())
        }),
    )
}

fn warn_unknown_clip(
    scene: &Scene,
    id: u32,
    clip: &str,
    call: &str,
    console: &RefCell<ConsoleLogs>,
) {
    let Some(mesh) = scene.world.mesh(id) else {
        return;
    };
    if mesh.clips.is_empty() || mesh.clips.iter().any(|c| c.name == clip) {
        return;
    }
    let msg = format!("Animator.{call}: entity {id}'s mesh has no clip '{clip}'");
    console.borrow_mut().repeated(msg, LogLevel::Warning, 1);
}
