use mlua::{Lua, Table};
use rusty_zimmer::level::Difference;

use super::level::{band_table, measure_file};

// `a` minus `b`: negative means `a` is quieter. A field is `nil` when a side has nothing to compare; `nil` is not zero.
pub fn diff(lua: &Lua, a: &str, b: &str) -> mlua::Result<Table> {
    let whole = |path: &str| {
        measure_file(path, None)
            .map(|profile| profile.whole)
            .map_err(mlua::Error::RuntimeError)
    };
    let difference = Difference::between(&whole(a)?, &whole(b)?);
    let table = lua.create_table()?;
    table.set("mean", difference.mean_db)?;
    table.set("peak", difference.peak_db)?;
    table.set("crest", difference.crest_db)?;
    table.set("correlation", difference.correlation)?;
    table.set("seconds", difference.seconds)?;
    if let Some(bands) = difference.bands {
        table.set("bands", band_table(lua, bands.low, bands.mid, bands.high)?)?;
    }
    table.set("same", difference.is_nothing())?;
    Ok(table)
}
