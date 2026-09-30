//! src/api/sound/mod.rs — the `Sound` namespace (#357, #358), a thin adapter over
//! scorsese's **zimmer** synthesiser (#413).
//!
//! Agent-facing surface for **procedural sound authoring**: describe an instrument
//! as a *patch* and **bake** one note of it to a stereo `.wav`. That single note is
//! the whole one-shot SFX story — a gunshot, an impact, a footstep and a UI blip
//! are each one rendered note of a noise / Karplus / FM patch — and the returned
//! path drops straight into `Audio.PlayAt` or an `AudioSource`'s `clip`, so a sound
//! the agent invented is audible in the same script that made it.
//!
//! The synthesis itself is not here: it is the `zimmer` crate, a git dependency
//! pinned to a commit in `Cargo.toml`. zimmer does no I/O, so this adapter owns the
//! two edges it leaves to its caller — writing the finished WAV to the requested
//! path, and resolving a song track's patch *reference* by reading that `.json` from
//! disk (`load_patch`). The document shapes (patch, song) are zimmer's serde
//! derives, so a Lua-authored patch and one loaded from `.json` are the same
//! document — one surface, three callers.
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
//! `{ duration, velocity, timbre, glide = { semitones, seconds }, seed }`, each
//! field optional. Same patch + note + seed ⇒ byte-identical WAV.
//!
//! `Sound` borrows no engine state — it reads a patch and writes a file — so it
//! registers as a plain static namespace, like `Texture` and `Shader`.

mod from_lua;

use mlua::{Lua, Table, Value};

use super::{put, Reg};
use from_lua::{note_from_value, opts_from_table, patch_from_table, song_from_table};
use zimmer::{Bake, Patch, Song};

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
                let patch = Patch::from_json(&json).map_err(lua_err)?;
                bake(&patch, &note, &path, opts.as_ref())
            },
        ),
    )?;

    put(
        &table,
        "ToJson",
        lua.create_function(|_, patch: Table| {
            let patch = patch_from_table(&patch).map_err(mlua::Error::RuntimeError)?;
            patch.to_json().map_err(lua_err)
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
            bake_song(&song, &path)
        }),
    )?;

    put(
        table,
        "BakeSongJson",
        lua.create_function(|_, (json, path): (String, String)| {
            let song = Song::from_json(&json).map_err(lua_err)?;
            bake_song(&song, &path)
        }),
    )?;

    put(
        table,
        "SongToJson",
        lua.create_function(|_, song: Table| {
            let song = song_from_table(&song).map_err(mlua::Error::RuntimeError)?;
            song.to_json().map_err(lua_err)
        }),
    )
}

/// Resolve the note + options, bake, and write — surfacing any error to Lua verbatim.
fn bake(patch: &Patch, note: &Value, path: &str, opts: Option<&Table>) -> mlua::Result<String> {
    let midi = note_from_value(note).map_err(mlua::Error::RuntimeError)?;
    let opts = opts_from_table(opts).map_err(mlua::Error::RuntimeError)?;
    write_bake(
        zimmer::bake_note(patch, midi, &opts).map_err(lua_err)?,
        path,
    )
}

/// Bake `song`, resolving any track that names its patch by reading that `.json`.
fn bake_song(song: &Song, path: &str) -> mlua::Result<String> {
    write_bake(zimmer::bake_song(song, &load_patch).map_err(lua_err)?, path)
}

/// The song resolver: a track's `patch` given as a string is a path to a saved
/// patch `.json`, read as-is (relative to the working directory). zimmer wraps the
/// failure with the track and reference that asked for it.
fn load_patch(path: &str) -> Result<Patch, String> {
    let json = std::fs::read_to_string(path).map_err(|e| format!("cannot read: {e}"))?;
    Patch::from_json(&json).map_err(|e| format!("invalid patch: {e}"))
}

/// Write a finished bake to `path`, creating its parent directory, and hand the
/// path back so it drops straight into `Audio.PlayAt`. Only reached once the render
/// succeeded, so a rejected recipe never leaves a file behind.
fn write_bake(bake: Bake, path: &str) -> mlua::Result<String> {
    if let Some(dir) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(dir).map_err(lua_err)?;
    }
    std::fs::write(path, bake.wav).map_err(lua_err)?;
    Ok(path.to_string())
}

/// Any displayable error (zimmer's `SynthError`, serde, I/O) as the Lua error the
/// script sees.
fn lua_err(e: impl std::fmt::Display) -> mlua::Error {
    mlua::Error::RuntimeError(e.to_string())
}

#[cfg(test)]
#[path = "bake_tests.rs"]
mod bake_tests;
