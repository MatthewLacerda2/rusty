use mlua::{Lua, Table, Value};

use super::from_lua::parse_recipe;
use super::put;
use crate::api::Reg;

use rusty_procgen::image_buf::Rgba;
use rusty_procgen::runner::preview::write_preview;
use rusty_procgen::runner::stats::{stats, NodeStats};
use rusty_procgen::TextureRecipe;

pub(super) fn register(lua: &Lua, table: &Table) -> Reg {
    put(
        table,
        "Stats",
        lua.create_function(|lua, (recipe, resolution): (Value, Option<u32>)| {
            let recipe = parse_recipe(&recipe).map_err(mlua::Error::RuntimeError)?;
            stats_table(lua, recipe, resolution)
        }),
    )?;
    put(
        table,
        "Preview",
        lua.create_function(|lua, (recipe, path, size): (Value, String, Option<u32>)| {
            let recipe = parse_recipe(&recipe).map_err(mlua::Error::RuntimeError)?;
            let root = rusty_core::project::root();
            preview_table(lua, &recipe, &path, size, &root)
        }),
    )
}

pub const PREVIEW_SIZE: u32 = 128;

pub fn stats_table(
    lua: &Lua,
    mut recipe: TextureRecipe,
    resolution: Option<u32>,
) -> mlua::Result<Table> {
    if let Some(r) = resolution {
        recipe
            .set_resolution(r)
            .map_err(|e| mlua::Error::RuntimeError(format!("Texture.Stats: {e}")))?;
    }
    let loaded = super::load(&recipe)?;
    let all = stats(&loaded.recipe, &loaded.sources)
        .map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
    let out = lua.create_table()?;
    for (node, s) in loaded.recipe.nodes.iter().zip(&all) {
        out.set(node.id.as_str(), node_table(lua, s)?)?;
    }
    Ok(out)
}

fn node_table(lua: &Lua, s: &NodeStats) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    let fields: [(&str, Rgba); 6] = [
        ("min", s.min),
        ("max", s.max),
        ("mean", s.mean),
        ("std", s.std),
        ("p1", s.p1),
        ("p99", s.p99),
    ];
    for (name, v) in fields {
        t.set(name, lua.create_sequence_from(v.map(f64::from))?)?;
    }
    Ok(t)
}

pub fn preview_table(
    lua: &Lua,
    recipe: &TextureRecipe,
    path: &str,
    size: Option<u32>,
    root: &std::path::Path,
) -> mlua::Result<Table> {
    let size = size.unwrap_or(PREVIEW_SIZE.min(recipe.extent().longer()));
    let loaded = super::load(recipe)?;
    let columns = super::folder::make_parents(path, root)
        .and_then(|()| write_preview(&loaded.recipe, &loaded.sources, path, size))
        .map_err(|e| mlua::Error::RuntimeError(format!("Texture.Preview: {e}")))?;
    let t = lua.create_table()?;
    t.set("path", path)?;
    t.set("columns", columns)?;
    let order = loaded.recipe.nodes.iter().map(|n| n.id.as_str());
    t.set("order", lua.create_sequence_from(order)?)?;
    Ok(t)
}
