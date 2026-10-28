//! src/api/texture/inspect.rs — `Texture.Stats` and `Texture.Preview` (#1486): let
//! the agent see what every node of a recipe makes, by numbers or by eye.

use mlua::{Lua, Table};

use crate::procgen::image_buf::Rgba;
use crate::procgen::runner::preview::write_preview;
use crate::procgen::runner::stats::{stats, NodeStats};
use crate::procgen::TextureRecipe;

/// The tile edge `Texture.Preview` uses when none is given, or the recipe's
/// resolution when that is smaller.
pub const PREVIEW_SIZE: u32 = 128;

/// `{ [node_id] = { min, max, mean, std, p1, p99 } }`, each an `{r, g, b, a}`
/// sequence, evaluated at `resolution` when given.
pub fn stats_table(
    lua: &Lua,
    mut recipe: TextureRecipe,
    resolution: Option<u32>,
) -> mlua::Result<Table> {
    if let Some(r) = resolution {
        recipe.resolution = r;
    }
    let all = stats(&recipe).map_err(|e| mlua::Error::RuntimeError(e.to_string()))?;
    let out = lua.create_table()?;
    for (node, s) in recipe.nodes.iter().zip(&all) {
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

/// Write the contact sheet and return `{ path, columns, order = { node ids } }`.
pub fn preview_table(
    lua: &Lua,
    recipe: &TextureRecipe,
    path: &str,
    size: Option<u32>,
) -> mlua::Result<Table> {
    let size = size.unwrap_or(PREVIEW_SIZE.min(recipe.resolution));
    let columns = write_preview(recipe, path, size)
        .map_err(|e| mlua::Error::RuntimeError(format!("Texture.Preview: {e}")))?;
    let t = lua.create_table()?;
    t.set("path", path)?;
    t.set("columns", columns)?;
    let order = recipe.nodes.iter().map(|n| n.id.as_str());
    t.set("order", lua.create_sequence_from(order)?)?;
    Ok(t)
}
