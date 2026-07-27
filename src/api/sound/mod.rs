//! src/api/sound/mod.rs — the `Sound` namespace (#357).
//!
//! Agent-facing surface for **procedural sound authoring**: describe an instrument
//! as a *patch* and **bake** one note of it to a mono `.wav`. That single note is
//! the whole one-shot SFX story — a gunshot, an impact, a footstep and a UI blip
//! are each one rendered note of a noise / Karplus / FM patch — and the returned
//! path drops straight into `Audio.PlayAt` or an `AudioSource`'s `clip`, so a sound
//! the agent invented is audible in the same script that made it.
//!
//! The patch is authored as a Lua table whose shape mirrors the serde document
//! exactly (see [`from_lua`]), so the same document describes a Lua-built patch and
//! one loaded from `.json` — one surface, three callers.
//!
//! Verbs:
//! - `Sound.Bake(patch, note, path [, opts])` — render + write; returns the path.
//! - `Sound.BakeJson(json, note, path [, opts])` — same, from a patch JSON string,
//!   so a saved patch re-bakes without a round-trip through Lua.
//! - `Sound.ToJson(patch)` — serialize a patch table to canonical JSON for saving
//!   or diffing.
//!
//! And the song verbs (#358), the same three shapes one level up — a *song* names
//! tracks, patterns and an arrangement, and bakes to one mixed WAV:
//! - `Sound.BakeSong(song, path)` / `Sound.BakeSongJson(json, path)` — render + write.
//! - `Sound.SongToJson(song)` — canonical JSON for saving or diffing.
//!
//! `note` is a name (`"C#4"`) or a MIDI number; `opts` is
//! `{ duration = seconds, velocity = 0..1, seed = integer }`, each field optional.
//! Same patch + note + seed ⇒ byte-identical WAV.
//!
//! `Sound` borrows no engine state — it reads a patch and writes a file — so it
//! registers as a plain static namespace, like `Texture` and `Shader`.

mod from_lua;

use mlua::{Lua, Table, Value};

use super::{put, Reg};
use crate::soundgen::{bake_note, bake_song, NoteOpts, Patch, Song};
use from_lua::{note_from_value, opts_from_table, patch_from_table, song_from_table};

/// Register the `Sound` namespace onto `lua`.
pub fn register(lua: &Lua) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "Bake",
        lua.create_function(
            |_, (patch, note, path, opts): (Table, Value, String, Option<Table>)| {
                let patch = patch_from_table(&patch).map_err(mlua::Error::RuntimeError)?;
                bake(&patch, &note, &path, opts.as_ref())
            },
        ),
    )?;

    put(
        &table,
        "BakeJson",
        lua.create_function(
            |_, (json, note, path, opts): (String, Value, String, Option<Table>)| {
                let patch = Patch::from_json(&json).map_err(mlua::Error::RuntimeError)?;
                bake(&patch, &note, &path, opts.as_ref())
            },
        ),
    )?;

    put(
        &table,
        "ToJson",
        lua.create_function(|_, patch: Table| {
            let patch = patch_from_table(&patch).map_err(mlua::Error::RuntimeError)?;
            patch.to_json().map_err(mlua::Error::RuntimeError)
        }),
    )?;

    register_song(lua, &table)?;

    lua.globals().set("Sound", table).map_err(|e| e.to_string())
}

/// The song verbs (#358): the same three shapes as the patch verbs above — from a
/// table, from JSON, and back to JSON — so a song is authored, saved and re-baked
/// exactly the way a patch is.
fn register_song(lua: &Lua, table: &Table) -> Reg {
    put(
        table,
        "BakeSong",
        lua.create_function(|_, (song, path): (Table, String)| {
            let song = song_from_table(&song).map_err(mlua::Error::RuntimeError)?;
            bake_song(&song, &path).map_err(mlua::Error::RuntimeError)
        }),
    )?;

    put(
        table,
        "BakeSongJson",
        lua.create_function(|_, (json, path): (String, String)| {
            let song = Song::from_json(&json).map_err(mlua::Error::RuntimeError)?;
            bake_song(&song, &path).map_err(mlua::Error::RuntimeError)
        }),
    )?;

    put(
        table,
        "SongToJson",
        lua.create_function(|_, song: Table| {
            let song = song_from_table(&song).map_err(mlua::Error::RuntimeError)?;
            song.to_json().map_err(mlua::Error::RuntimeError)
        }),
    )
}

/// Resolve the note + options and bake, surfacing any error to Lua verbatim.
fn bake(patch: &Patch, note: &Value, path: &str, opts: Option<&Table>) -> mlua::Result<String> {
    let midi = note_from_value(note).map_err(mlua::Error::RuntimeError)?;
    let opts: NoteOpts = opts_from_table(opts).map_err(mlua::Error::RuntimeError)?;
    bake_note(patch, midi, &opts, path).map_err(mlua::Error::RuntimeError)
}

#[cfg(test)]
#[path = "bake_tests.rs"]
mod bake_tests;
