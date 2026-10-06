//! src/dev/lua_surface/voices.rs — `Speech.Voices`, `Speech.FindVoice`,
//! `Speech.VoiceInfo` (#387): where the `voice` of a speech brief comes from.
//!
//! Added to the `Speech` table [`super::speech`] creates, so it registers after it.
//! Free (no budget, no ledger), still `dev`-only and refused during Play.

use std::cell::RefCell;
use std::path::Path;

use mlua::{Lua, Table, Value};

use super::super::providers::environment;
use super::super::providers::voices::{self, Account, Answer, Availability, Voice, REFRESH};
use crate::api::{global_table, put, ApiScopedCtx, Reg};

/// `Voices([refresh])` and `FindVoice(query [, refresh])` → `voices, source`, where
/// `source` says whether the list was just read or cached and how old it is.
/// `VoiceInfo(id)` → the voice, or `nil, why` when it is withdrawn. Anything that
/// stops every question (Play, no key on a cold cache, the vendor) raises.
pub(super) fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    let playing: &'scope RefCell<bool> = ctx.is_playing;
    let table = global_table(lua, "Speech")?;
    put(
        &table,
        "Voices",
        scope.create_function(move |lua, refresh: Option<bool>| {
            let env = environment();
            let account = Account { environment: &env };
            let answer = voices::list(
                Path::new(""),
                *playing.borrow(),
                &account,
                refresh.unwrap_or(false),
            );
            listing(lua, answer.map_err(mlua::Error::external)?)
        }),
    )?;
    put(
        &table,
        "FindVoice",
        scope.create_function(move |lua, (query, refresh): (String, Option<bool>)| {
            let env = environment();
            let account = Account { environment: &env };
            let at = (Path::new(""), *playing.borrow());
            let answer = voices::find(at.0, at.1, &account, &query, refresh.unwrap_or(false));
            listing(lua, answer.map_err(mlua::Error::external)?)
        }),
    )?;
    put(
        &table,
        "VoiceInfo",
        scope.create_function(move |lua, id: String| {
            let env = environment();
            let account = Account { environment: &env };
            match voices::info(*playing.borrow(), &account, &id).map_err(mlua::Error::external)? {
                Availability::Available(voice) => {
                    Ok((Value::Table(voice_table(lua, &voice)?), None))
                }
                Availability::Unusable(why) => Ok((Value::Nil, Some(voices::withdrawn(&id, &why)))),
            }
        }),
    )
}

/// The voices as a Lua list, and the line saying where they came from.
fn listing(lua: &Lua, answer: Answer) -> mlua::Result<(Table, String)> {
    let list = lua.create_table()?;
    for voice in &answer.voices {
        list.push(voice_table(lua, voice)?)?;
    }
    Ok((list, answer.summary("ElevenLabs", REFRESH)))
}

/// `{ id, name, traits = {…}, description, preview }`.
fn voice_table(lua: &Lua, voice: &Voice) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("id", voice.id.as_str())?;
    t.set("name", voice.name.as_str())?;
    t.set(
        "traits",
        lua.create_sequence_from(voice.traits.iter().map(String::as_str))?,
    )?;
    t.set("description", voice.description.as_deref())?;
    t.set("preview", voice.preview.as_deref())?;
    Ok(t)
}
