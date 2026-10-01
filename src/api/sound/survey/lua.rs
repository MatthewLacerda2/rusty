//! src/api/sound/survey/lua.rs — `Sound.Survey`'s two Lua edges: the list of
//! documents in, the report table out (#380).
//!
//! In: a sequence whose entries are each a **path** to a patch or song `.json`, or
//! an **inline document** — a table, or a JSON string (one starting with `{`),
//! decoded by the same `recipe_json` every `Sound` verb uses (#410). Inline entries
//! are named `#n` by their position.
//!
//! Out: `{ patches, songs, skipped, rollup }`, flat keyed tables like the level
//! table, so every row reads the way a bake's report does. `rollup` is `nil` for a
//! set of fewer than two.

use mlua::{Lua, Table, Value};
use serde_json::Value as Json;
use zimmer::survey::{Register, SongSurvey, TrackSurvey};

use super::{Document, PatchSurvey, SetRollup, SetSurvey, SourceRow};
use crate::api::lua_json::recipe_json;

/// Survey the documents `set` lists and build the report table.
pub fn survey<'lua>(lua: &'lua Lua, set: Table<'lua>) -> mlua::Result<Table<'lua>> {
    let documents = set
        .sequence_values::<Value>()
        .enumerate()
        .map(|(i, entry)| Ok(document(i + 1, &entry?)))
        .collect::<mlua::Result<Vec<_>>>()?;
    let set = SetSurvey::of(documents, &super::super::load_patch);
    let out = lua.create_table()?;
    out.set("patches", rows(lua, &set.patches, patch_row)?)?;
    out.set("songs", rows(lua, &set.songs.songs, song_row)?)?;
    out.set(
        "skipped",
        rows(lua, &set.skipped, |lua, (name, error)| {
            let row = lua.create_table()?;
            row.set("name", name.as_str())?;
            row.set("error", error.as_str())?;
            Ok(row)
        })?,
    )?;
    out.set("rollup", set.rollup().map(|r| rollup(lua, &r)).transpose()?)?;
    Ok(out)
}

/// One list entry as a named document: a path is read from disk, anything else is
/// an inline document. A failure is carried, not raised — it lands in `skipped`.
fn document(position: usize, entry: &Value) -> Document {
    match entry {
        Value::String(s) => match s.to_str() {
            Ok(text) if text.trim_start().starts_with('{') => {
                (format!("#{position}"), recipe_json(entry))
            }
            Ok(path) => (path.to_owned(), read(path)),
            Err(e) => (format!("#{position}"), Err(e.to_string())),
        },
        other => (format!("#{position}"), recipe_json(other)),
    }
}

fn read(path: &str) -> Result<Json, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("invalid JSON: {e}"))
}

fn rows<'lua, T>(
    lua: &'lua Lua,
    items: &[T],
    row: impl Fn(&'lua Lua, &T) -> mlua::Result<Table<'lua>>,
) -> mlua::Result<Table<'lua>> {
    lua.create_sequence_from(
        items
            .iter()
            .map(|item| row(lua, item))
            .collect::<mlua::Result<Vec<_>>>()?,
    )
}

fn patch_row<'lua>(lua: &'lua Lua, patch: &PatchSurvey) -> mlua::Result<Table<'lua>> {
    let row = lua.create_table()?;
    row.set("name", patch.name.as_str())?;
    row.set("source", patch.source)?;
    row.set("filter", patch.filter.as_deref())?;
    row.set("cutoff", patch.cutoff)?;
    row.set("sustain", patch.sustain)?;
    Ok(row)
}

fn song_row<'lua>(lua: &'lua Lua, song: &SongSurvey) -> mlua::Result<Table<'lua>> {
    let row = lua.create_table()?;
    row.set("name", song.name.as_str())?;
    row.set("bpm", song.bpm)?;
    row.set("seconds", song.seconds)?;
    row.set(
        "register",
        song.register.map(|r| register(lua, r)).transpose()?,
    )?;
    row.set("loudest", song.loudest().map(|t| t.name.as_str()))?;
    let seconds = song.seconds;
    row.set(
        "tracks",
        rows(lua, &song.tracks, |lua, t| track_row(lua, t, seconds))?,
    )?;
    Ok(row)
}

fn track_row<'lua>(lua: &'lua Lua, track: &TrackSurvey, seconds: f32) -> mlua::Result<Table<'lua>> {
    let row = lua.create_table()?;
    row.set("name", track.name.as_str())?;
    row.set("source", track.source)?;
    row.set("gain", track.gain)?;
    row.set("cutoff", track.cutoff)?;
    row.set("sustain", track.sustain)?;
    row.set("notes", track.notes)?;
    row.set("median", track.median)?;
    row.set("duty", track.duty)?;
    row.set("density", track.density(seconds))?;
    Ok(row)
}

fn rollup<'lua>(lua: &'lua Lua, rollup: &SetRollup) -> mlua::Result<Table<'lua>> {
    let out = lua.create_table()?;
    out.set("patches", rollup.patches)?;
    out.set("songs", rollup.songs)?;
    out.set("sources", rows(lua, &rollup.sources, source_row)?)?;
    out.set(
        "tempo",
        rollup.tempo.map(|span| range(lua, span)).transpose()?,
    )?;
    out.set(
        "register",
        rollup.register.map(|r| register(lua, r)).transpose()?,
    )?;
    Ok(out)
}

fn source_row<'lua>(lua: &'lua Lua, source: &SourceRow) -> mlua::Result<Table<'lua>> {
    let row = lua.create_table()?;
    row.set("source", source.source)?;
    row.set("patches", source.patches)?;
    row.set("songs", source.songs)?;
    row.set("loudest", source.loudest)?;
    row.set(
        "cutoff",
        source.cutoff.map(|span| range(lua, span)).transpose()?,
    )?;
    Ok(row)
}

/// A `{ low, high }` span.
fn range(lua: &Lua, (low, high): (f32, f32)) -> mlua::Result<Table<'_>> {
    let span = lua.create_table()?;
    span.set("low", low)?;
    span.set("high", high)?;
    Ok(span)
}

/// A register as `{ low, high }` in MIDI, plus its note names (`"E1-A5"`).
fn register(lua: &Lua, register: Register) -> mlua::Result<Table<'_>> {
    let span = range(lua, (register.low, register.high))?;
    span.set("names", register.to_string())?;
    Ok(span)
}
