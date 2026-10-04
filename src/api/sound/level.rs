//! src/api/sound/level.rs — how a sound came out, as the Lua table the `Sound`
//! verbs hand back (#378, #379).
//!
//! **The agent cannot hear**, so a bake reports its own level: every bake verb
//! returns this table as its second value, and `Sound.Level(path)` measures any
//! clip on disk the same way. The arithmetic is zimmer's (`zimmer::level`) — a bake
//! arrives already measured, and a file is decoded by the engine's own `ClipCache`
//! decoder and fed to zimmer's `Profiler` — so there is one meter, not two.
//!
//! A **signal, never a gate**: there is no correct loudness, so nothing here
//! refuses or fails. The two cases that are not taste — a silence and a true peak
//! at or over full scale — are flagged (`silent`, `clipping`), still not errors.
//!
//! A **row** — the whole clip, one section of it, or one track — is always the same
//! fields: `mean`, `peak`, `true_peak` (dBFS), `crest` (dB, peak − mean),
//! `seconds`, `silent`, `clipping`, `bands` (`{ low, mid, high }` whole percentages
//! of the energy, split at 250 Hz and 4 kHz) and `correlation` (`-1..1`, how much
//! of the signal both channels share). The levels and `bands` are `nil` for a
//! silence, `correlation` for anything not two live channels.
//!
//! The **report** is the whole-clip row plus two lists of rows (#379):
//! `sections` (each with `label`, `from`, `to`) — the arrangement's patterns for a
//! song, an 8-second grid otherwise — and `tracks` (each with `name` and its own
//! `sections`), post-gain, for a song of more than one track. Both lists are empty
//! where a row would only repeat the summary.

use mlua::{Lua, Table};

use crate::audio::device::decode::decode_file;
use zimmer::level::{Layer, Profile, Profiler, Span};

/// Measure the clip at `path` over the PCM the engine's decoder produces for it —
/// any format `ClipCache` plays (`.wav`, `.ogg`, `.mp3`). A file carries no
/// arrangement, so its sections fall on zimmer's fixed grid.
pub fn measure_file(path: &str) -> Result<Profile, String> {
    let clip = decode_file(path).map_err(|e| format!("cannot decode '{path}': {e}"))?;
    let mut profiler = Profiler::new(usize::from(clip.channels()), clip.sample_rate());
    profiler.feed(clip.samples());
    Ok(profiler.finish())
}

/// A whole report: the clip's row, its `sections`, and its `tracks`.
pub fn report_table(lua: &Lua, profile: &Profile, tracks: &[Layer]) -> mlua::Result<Table> {
    let table = level_table(lua, &profile.whole)?;
    table.set("sections", sections_table(lua, &profile.sections)?)?;
    let rows = lua.create_table()?;
    for track in tracks {
        let row = level_table(lua, &track.level)?;
        row.set("name", track.name.as_str())?;
        row.set("sections", sections_table(lua, &track.sections)?)?;
        rows.push(row)?;
    }
    table.set("tracks", rows)?;
    Ok(table)
}

/// Section rows: a level row each, plus where it sits and what the song calls it.
fn sections_table(lua: &Lua, sections: &[Span]) -> mlua::Result<Table> {
    let rows = lua.create_table()?;
    for span in sections {
        let row = level_table(lua, span)?;
        row.set("label", span.label.as_deref())?;
        row.set("from", span.from_seconds)?;
        row.set("to", span.to_seconds)?;
        rows.push(row)?;
    }
    Ok(rows)
}

/// One measured span as the level row described in the module doc.
fn level_table(lua: &Lua, span: &Span) -> mlua::Result<Table> {
    let loudness = &span.loudness;
    let table = lua.create_table()?;
    table.set("mean", loudness.mean_dbfs)?;
    table.set("peak", loudness.peak_dbfs)?;
    table.set("true_peak", loudness.true_peak_dbfs)?;
    table.set("crest", span.crest_db())?;
    table.set("seconds", span.seconds())?;
    table.set("silent", loudness.is_silent())?;
    table.set("clipping", loudness.is_clipping())?;
    if let Some(bands) = span.bands {
        let (low, mid, high) = bands.percentages();
        table.set("bands", band_table(lua, low, mid, high)?)?;
    }
    table.set("correlation", span.correlation)?;
    Ok(table)
}

/// `{ low, mid, high }` — shares in percent, or a diff's moves in points.
pub fn band_table<T: mlua::IntoLua>(lua: &Lua, low: T, mid: T, high: T) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("low", low)?;
    table.set("mid", mid)?;
    table.set("high", high)?;
    Ok(table)
}
