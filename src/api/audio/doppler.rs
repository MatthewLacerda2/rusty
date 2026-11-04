//! src/api/audio/doppler.rs — the `Audio` doppler verbs (#1749).
//!
//! The per-source level (`AudioSource.doppler_level`, the authoring op the
//! inspector shares), the running speed of sound, and the rate a source's voice
//! plays at now, for a play-test to assert a pass-by bends the pitch.

use mlua::Table;

use super::super::{put, Reg};
use super::groups::err;
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::audio::AudioMaestro;
use crate::scene::authoring::audio::source as audio_ops;
use crate::scene::Scene;

/// `SetDopplerLevel` / `GetDopplerLevel` / `GetDoppler`, and the speed of sound.
pub(super) fn register_doppler(
    lua: &mlua::Lua,
    table: &Table,
    scene: ApiCell<Scene>,
    audio: ApiCell<AudioMaestro>,
) -> Reg {
    put(
        table,
        "SetDopplerLevel",
        lua.create_function(move |lua, (id, level): (EntityArg, f32)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Audio", "SetDopplerLevel")?;
            let mut scene = scene.borrow_mut();
            let mut source = scene
                .world
                .audio_mut(id)
                .ok_or_else(|| err(format!("entity {id} has no AudioSource")))?;
            audio_ops::set_doppler_level(&mut source, level);
            Ok(())
        }),
    )?;
    put(
        table,
        "GetDopplerLevel",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            Ok(scene.world.audio(id.read()).map(|a| a.doppler_level))
        }),
    )?;
    put(
        table,
        "GetDoppler",
        lua.create_function(move |lua, id: EntityArg| {
            Ok(audio.get(lua)?.borrow().source_doppler(id.read()))
        }),
    )?;
    register_speed_of_sound(lua, table, audio)
}

/// `SetSpeedOfSound` / `GetSpeedOfSound`: the running game's, until Stop puts the
/// project's back.
fn register_speed_of_sound(lua: &mlua::Lua, table: &Table, audio: ApiCell<AudioMaestro>) -> Reg {
    put(
        table,
        "SetSpeedOfSound",
        lua.create_function(move |lua, c: f32| {
            if audio.get(lua)?.borrow_mut().set_speed_of_sound(c) {
                Ok(())
            } else {
                Err(err(format!(
                    "speed of sound must be a positive number, got {c}"
                )))
            }
        }),
    )?;
    put(
        table,
        "GetSpeedOfSound",
        lua.create_function(move |lua, ()| Ok(audio.get(lua)?.borrow().speed_of_sound())),
    )
}
