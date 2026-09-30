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
//! - `Sound.Bake(patch, note, path [, opts])` — render + write; returns the path
//!   and how loud it came out (#378, `level.rs`).
//! - `Sound.ToJson(patch)` — serialize a patch to canonical JSON for saving or
//!   diffing.
//!
//! And the song verbs (#358), the same shapes one level up — a *song* names
//! tracks, patterns and an arrangement, and bakes to one mixed WAV:
//! - `Sound.BakeSong(song, path)` — render + write.
//! - `Sound.SongToJson(song)` — canonical JSON for saving or diffing.
//!
//! A `patch` or `song` is a Lua table **or** its JSON string (#410), so a saved
//! document re-bakes through the same verb without a round-trip through Lua.
//!
//! Every bake verb returns `path, level`: the path first, because callers feed it
//! straight to `Audio.PlayAt`, and the level table second, so the figures are never
//! one forgotten call away. `Sound.Level(path)` measures any clip on disk the same
//! way — an import, or an earlier bake being compared against (#378).
//!
//! `note` is a name (`"C#4"`) or a MIDI number; `opts` is
//! `{ duration, velocity, timbre, glide = { semitones, seconds }, seed }`, each
//! field optional. Same patch + note + seed ⇒ byte-identical WAV.
//!
//! `Sound` borrows no engine state — it reads a patch and writes a file — so it
//! registers as a plain static namespace, like `Texture` and `Shader`.

mod from_lua;
mod level;

use mlua::{Lua, Table, Value};

use super::{put, Reg};
use from_lua::{note_from_value, opts_from_table, patch_from_lua, song_from_lua};
use level::{level_table, measure_file};
use zimmer::{Bake, Patch, Song};

/// Register the `Sound` namespace onto `lua`.
pub fn register(lua: &Lua) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "Bake",
        lua.create_function(
            |lua, (patch, note, path, opts): (Value, Value, String, Option<Table>)| {
                let patch = patch_from_lua(&patch).map_err(mlua::Error::RuntimeError)?;
                bake(lua, &patch, &note, &path, opts.as_ref())
            },
        ),
    )?;

    put(
        &table,
        "ToJson",
        lua.create_function(|_, patch: Value| {
            let patch = patch_from_lua(&patch).map_err(mlua::Error::RuntimeError)?;
            patch.to_json().map_err(lua_err)
        }),
    )?;

    put(
        &table,
        "Level",
        lua.create_function(|lua, path: String| {
            level_table(
                lua,
                &measure_file(&path).map_err(mlua::Error::RuntimeError)?,
            )
        }),
    )?;

    register_song(lua, &table)?;

    lua.globals().set("Sound", table).map_err(|e| e.to_string())
}

/// The song verbs (#358): the same shapes as the patch verbs above — bake from a
/// table or JSON, and back to JSON — so a song is authored, saved and re-baked
/// exactly the way a patch is.
fn register_song(lua: &Lua, table: &Table) -> Reg {
    put(
        table,
        "BakeSong",
        lua.create_function(|lua, (song, path): (Value, String)| {
            let song = song_from_lua(&song).map_err(mlua::Error::RuntimeError)?;
            bake_song(lua, &song, &path)
        }),
    )?;

    put(
        table,
        "SongToJson",
        lua.create_function(|_, song: Value| {
            let song = song_from_lua(&song).map_err(mlua::Error::RuntimeError)?;
            song.to_json().map_err(lua_err)
        }),
    )
}

/// Resolve the note + options, bake, and write — surfacing any error to Lua verbatim.
fn bake<'lua>(
    lua: &'lua Lua,
    patch: &Patch,
    note: &Value,
    path: &str,
    opts: Option<&Table>,
) -> Baked<'lua> {
    let midi = note_from_value(note).map_err(mlua::Error::RuntimeError)?;
    let opts = opts_from_table(opts).map_err(mlua::Error::RuntimeError)?;
    let bake = zimmer::bake_note(patch, midi, &opts).map_err(lua_err)?;
    write_bake(lua, bake, path)
}

/// Bake `song`, resolving any track that names its patch by reading that `.json`.
fn bake_song<'lua>(lua: &'lua Lua, song: &Song, path: &str) -> Baked<'lua> {
    let bake = zimmer::bake_song(song, &load_patch).map_err(lua_err)?;
    write_bake(lua, bake, path)
}

/// The song resolver: a track's `patch` given as a string is a path to a saved
/// patch `.json`, read as-is (relative to the working directory). zimmer wraps the
/// failure with the track and reference that asked for it.
fn load_patch(path: &str) -> Result<Patch, String> {
    let json = std::fs::read_to_string(path).map_err(|e| format!("cannot read: {e}"))?;
    Patch::from_json(&json).map_err(|e| format!("invalid patch: {e}"))
}

/// What every bake verb returns to Lua: the written path, then its level table.
type Baked<'lua> = mlua::Result<(String, Table<'lua>)>;

/// Write a finished bake to `path`, creating its parent directory, and hand back
/// the path (so it drops straight into `Audio.PlayAt`) with how loud it came out —
/// zimmer measured the samples before encoding, so this costs no second pass. Only
/// reached once the render succeeded, so a rejected recipe never leaves a file behind.
fn write_bake<'lua>(lua: &'lua Lua, bake: Bake, path: &str) -> Baked<'lua> {
    if let Some(dir) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(dir).map_err(lua_err)?;
    }
    std::fs::write(path, &bake.wav).map_err(lua_err)?;
    Ok((path.to_string(), level_table(lua, &bake.profile.whole)?))
}

/// Any displayable error (zimmer's `SynthError`, serde, I/O) as the Lua error the
/// script sees.
fn lua_err(e: impl std::fmt::Display) -> mlua::Error {
    mlua::Error::RuntimeError(e.to_string())
}

#[cfg(test)]
#[path = "bake_tests.rs"]
mod bake_tests;

#[cfg(test)]
#[path = "level_tests.rs"]
mod level_tests;
