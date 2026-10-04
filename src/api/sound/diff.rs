//! src/api/sound/diff.rs — `Sound.Diff(a, b)`: two clips compared field by field
//! (#379).
//!
//! An absolute level is hard to judge; "4.6 dB under the version it replaces" is a
//! finding. Both sides are measured the way `Sound.Level` measures a file, and the
//! subtraction is zimmer's `Difference` — `a` minus `b`, so negative means `a` is
//! quieter. A signal, never a gate.
//!
//! The table: `mean`, `peak`, `crest` (dB), `correlation` (positive means `a` is
//! *narrower*), `seconds`, `bands` (`{ low, mid, high }` in percentage **points**),
//! and `same` — `true` when nothing measurably moved. A field is `nil` when either
//! side had nothing to compare (a silence has no level and no balance): `nil` is not
//! zero, zero says the two matched.

use mlua::{Lua, Table};
use zimmer::level::Difference;

use super::level::{band_table, measure_file};

/// Measure the clips at `a` and `b` and report how `a` differs from `b`.
pub fn diff(lua: &Lua, a: &str, b: &str) -> mlua::Result<Table> {
    let whole = |path: &str| {
        measure_file(path)
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
