//! src/api/sound/level.rs — how loud a sound came out, as the Lua table the
//! `Sound` verbs hand back (#378).
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
//! The table: `mean`, `peak`, `true_peak` (dBFS), `crest` (dB, peak − mean),
//! `seconds`, `silent`, `clipping`. The four levels are `nil` for a silence — a
//! clip that makes no sound, not one at minus infinity.

use mlua::{Lua, Table};

use crate::audio::device::decode::decode_file;
use zimmer::level::{Profiler, Span};

/// Full scale of a decoded 16-bit sample, as the divisor that maps it to `-1..1`.
const I16_FULL_SCALE: f32 = 32_768.0;

/// Measure the clip at `path` over the PCM the engine's decoder produces for it —
/// any format `ClipCache` plays (`.wav`, `.ogg`, `.mp3`).
pub fn measure_file(path: &str) -> Result<Span, String> {
    let clip = decode_file(path).map_err(|e| format!("cannot decode '{path}': {e}"))?;
    let samples: Vec<f32> = clip
        .samples()
        .iter()
        .map(|&s| f32::from(s) / I16_FULL_SCALE)
        .collect();
    let mut profiler = Profiler::new(usize::from(clip.channels()), clip.sample_rate());
    profiler.feed(&samples);
    Ok(profiler.finish().whole)
}

/// One measured span as the Lua level table described in the module doc.
pub fn level_table<'lua>(lua: &'lua Lua, span: &Span) -> mlua::Result<Table<'lua>> {
    let loudness = &span.loudness;
    let table = lua.create_table()?;
    table.set("mean", loudness.mean_dbfs)?;
    table.set("peak", loudness.peak_dbfs)?;
    table.set("true_peak", loudness.true_peak_dbfs)?;
    table.set("crest", span.crest_db())?;
    table.set("seconds", span.seconds())?;
    table.set("silent", loudness.is_silent())?;
    table.set("clipping", loudness.is_clipping())?;
    Ok(table)
}
