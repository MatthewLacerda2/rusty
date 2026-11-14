//! crates/dev/src/lua_surface/debug/audio.rs — `Debug.AudioEvents` / `Debug.AudioVoices`
//! (#1247): what the agent hears, read back without a sound card.
//!
//! The `AudioMaestro` keeps an event log of every play action and can build a roster
//! row per `AudioSource` (`audio::introspection`, #212). `Debug.Snapshot` shows the
//! `AudioSource` *components*, so a fire-and-forget `Audio.PlayAt` one-shot, which
//! leaves none, is visible only here. Both reads hand the records to Lua through
//! serde (`lua_json::to_lua`), so the tables carry exactly the Rust fields and cannot
//! drift from them.

use mlua::{Table, Value};

use rusty_script::api::audio::{emitter, listener};
use rusty_script::api::lua_json::to_lua;
use rusty_script::api::{put, ApiCtx, Reg};

/// Register both audio reads on the `Debug` table.
pub fn register(lua: &mlua::Lua, table: &Table, ctx: &ApiCtx) -> Reg {
    let audio = ctx.audio;
    // The cursor is the last `seq` a reader saw; the second return is the oldest
    // `seq` still held, so a reader can tell when events rolled off past its cursor.
    put(
        table,
        "AudioEvents",
        lua.create_function(move |lua, after_seq: Option<u64>| {
            let audio = &*audio.get(lua)?;
            let audio = audio.borrow();
            let (events, oldest) = audio.events_after(after_seq.unwrap_or(0));
            Ok((to_lua(lua, events)?, oldest))
        }),
    )?;

    let (scene, camera) = (ctx.scene, ctx.camera);
    put(
        table,
        "AudioVoices",
        lua.create_function(move |lua, ()| -> mlua::Result<Value> {
            let audio = &*audio.get(lua)?;
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let (audio, heard_by) = (audio.borrow(), listener(&*camera.get(lua)?, &scene));
            let voices: Vec<_> = scene
                .world
                .ids_with_audio()
                .into_iter()
                .filter_map(|id| {
                    let (source, at) = emitter(&scene, id)?;
                    Some(audio.voice_info(id, &source, at, &heard_by))
                })
                .collect();
            to_lua(lua, &voices)
        }),
    )
}

#[cfg(test)]
#[path = "audio_tests.rs"]
mod tests;
