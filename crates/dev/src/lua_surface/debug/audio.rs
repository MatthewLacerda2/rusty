use mlua::{Table, Value};

use rusty_script::api::audio::{emitter, listener};
use rusty_script::api::lua_json::to_lua;
use rusty_script::api::{put, ApiCtx, Reg};

pub fn register(lua: &mlua::Lua, table: &Table, ctx: &ApiCtx) -> Reg {
    let audio = ctx.audio;
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
