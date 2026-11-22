mod diff;
mod from_lua;
mod level;
mod survey;

use mlua::{Lua, Table, Value};

use super::{put, Reg};
use from_lua::{note_from_value, opts_from_table, patch_from_lua, song_from_lua};
use level::{measure_file, report_table};
use rusty_zimmer::{Bake, Patch, Song};

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
        lua.create_function(|lua, (path, opts): (String, Option<Table>)| {
            let grid = match opts {
                Some(opts) => opts.get::<Option<f64>>("sections")?,
                None => None,
            };
            let profile = measure_file(&path, grid).map_err(mlua::Error::RuntimeError)?;
            report_table(lua, &profile, &[])
        }),
    )?;

    put(
        &table,
        "Diff",
        lua.create_function(|lua, (a, b): (String, String)| diff::diff(lua, &a, &b)),
    )?;

    put(
        &table,
        "Survey",
        lua.create_function(|lua, set: Table| survey::survey(lua, set)),
    )?;

    register_song(lua, &table)?;

    lua.globals().set("Sound", table).map_err(|e| e.to_string())
}

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

fn bake(lua: &Lua, patch: &Patch, note: &Value, path: &str, opts: Option<&Table>) -> Baked {
    let midi = note_from_value(note).map_err(mlua::Error::RuntimeError)?;
    let opts = opts_from_table(opts).map_err(mlua::Error::RuntimeError)?;
    let bake = rusty_zimmer::bake_note(patch, midi, &opts).map_err(lua_err)?;
    write_bake(lua, bake, path)
}

fn bake_song(lua: &Lua, song: &Song, path: &str) -> Baked {
    let bake = rusty_zimmer::bake_song(song, &load_patch).map_err(lua_err)?;
    write_bake(lua, bake, path)
}

fn load_patch(path: &str) -> Result<Patch, String> {
    let json = std::fs::read_to_string(path).map_err(|e| format!("cannot read: {e}"))?;
    Patch::from_json(&json).map_err(|e| format!("invalid patch: {e}"))
}

type Baked = mlua::Result<(String, Table)>;

fn write_bake(lua: &Lua, bake: Bake, path: &str) -> Baked {
    if let Some(dir) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(dir).map_err(lua_err)?;
    }
    // Through `fs::written`, so a clip cache that already played this path decodes the new file.
    rusty_core::fs::written::write(std::path::Path::new(path), &bake.wav).map_err(lua_err)?;
    Ok((
        path.to_string(),
        report_table(lua, &bake.profile, &bake.tracks)?,
    ))
}

fn lua_err(e: impl std::fmt::Display) -> mlua::Error {
    mlua::Error::RuntimeError(e.to_string())
}

#[cfg(test)]
#[path = "fixture.rs"]
mod fixture;

#[cfg(test)]
#[path = "bake_tests.rs"]
mod bake_tests;

#[cfg(test)]
#[path = "rebake_tests.rs"]
mod rebake_tests;

#[cfg(test)]
#[path = "grid_tests.rs"]
mod grid_tests;

#[cfg(test)]
#[path = "level_tests.rs"]
mod level_tests;

#[cfg(test)]
#[path = "report_tests.rs"]
mod report_tests;

#[cfg(test)]
#[path = "tail_tests.rs"]
mod tail_tests;
