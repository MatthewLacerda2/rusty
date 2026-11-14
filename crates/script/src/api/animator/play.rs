//! `Animator.Play` / `Animator.Crossfade`: start a clip on the base layer, and
//! warn when the entity's mesh has no clip of that name (#1718).

use std::cell::RefCell;

use mlua::Lua;

use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::scripting::{ConsoleLogs, LogLevel};
use rusty_scene::Scene;

use super::{put, Reg};

/// `Play` hard-cuts to a clip; `Crossfade` blends into it over its duration (#80),
/// degrading to a play for a non-positive duration or the clip already playing.
/// Neither checks the clip: an unknown one still replaces the current motion and
/// resets the playhead (a clip that resolves late, #1386). A normal call logs
/// nothing, as Unity's does (#1718), so a weapon restarting its fire clip 20 times a
/// second cannot flood the console; a name the entity's mesh lacks warns.
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

/// Warn when `id`'s mesh carries clips and none is named `clip` (Unity warns on a
/// state it can't find). No mesh, or one whose clips haven't loaded, stays quiet, so
/// a `Play` issued before the mesh arrives is not a false alarm (#1386). Repeats
/// collapse into one counted line.
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
