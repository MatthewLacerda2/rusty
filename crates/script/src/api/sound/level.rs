use mlua::{Lua, Table};

use rusty_audio::device::decode::decode_file;
use rusty_zimmer::level::{Cut, Layer, Profile, Profiler, Span};

pub const MAX_SECTIONS: f64 = 10_000.0;

pub fn measure_file(path: &str, grid: Option<f64>) -> Result<Profile, String> {
    let clip = decode_file(path).map_err(|e| format!("cannot decode '{path}': {e}"))?;
    let (channels, rate) = (usize::from(clip.channels()), clip.sample_rate());
    let seconds = clip.samples().len() as f64 / (channels.max(1) as f64 * f64::from(rate));
    let cuts = match grid {
        Some(every) => grid_cuts(every, seconds)?,
        None => Vec::new(),
    };
    let mut profiler = Profiler::sectioned(channels, rate, cuts);
    profiler.feed(clip.samples());
    let mut profile = profiler.finish();
    // A grid row is unlabelled, like zimmer's own grid; the cuts only carried a name
    // because a `Cut` must.
    profile.sections.iter_mut().for_each(|s| s.label = None);
    Ok(profile)
}

fn grid_cuts(every: f64, seconds: f64) -> Result<Vec<Cut>, String> {
    if !(every.is_finite() && every > 0.0) {
        return Err(format!(
            "sections must be a positive number of seconds, not {every}"
        ));
    }
    let count = (seconds / every).ceil();
    if count > MAX_SECTIONS {
        return Err(format!(
            "sections of {every}s cut this clip into over {MAX_SECTIONS} rows"
        ));
    }
    let cut = |k: u32| Cut {
        label: String::new(),
        end_seconds: f64::from(k) * every,
    };
    Ok((1..=count as u32).map(cut).collect())
}

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

pub fn band_table<T: mlua::IntoLua>(lua: &Lua, low: T, mid: T, high: T) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("low", low)?;
    table.set("mid", mid)?;
    table.set("high", high)?;
    Ok(table)
}
