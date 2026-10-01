//! src/api/shader/blocks.rs — `Shader.Blocks(pass)`: the block catalog as Lua
//! tables (#411).
//!
//! Read straight from [`crate::shadergen::blocks::catalog`] — the same catalog the
//! assembler and the strict param check (#395) resolve against — so what the agent
//! reads is exactly what a recipe may name.

use mlua::{Lua, Table, Value};

use crate::shadergen::blocks::{catalog, Block, Param};
use crate::shadergen::recipe::PassKind;

/// `{ {id, desc, params = { {name, default, arity, runtime} }, textures = {slot…} }, … }`
/// for `pass`. `textures` lists the extra texture slots the block samples (#400) —
/// the `shader_textures` keys a material using it should name.
pub fn blocks_table(lua: &Lua, pass: PassKind) -> mlua::Result<Table<'_>> {
    lua.create_sequence_from(
        catalog(pass)
            .iter()
            .map(|b| block_table(lua, b))
            .collect::<mlua::Result<Vec<_>>>()?,
    )
}

fn block_table<'lua>(lua: &'lua Lua, b: &Block) -> mlua::Result<Table<'lua>> {
    let t = lua.create_table()?;
    t.set("id", b.id)?;
    t.set("desc", b.desc)?;
    let params = b
        .params
        .iter()
        .map(|p| param_table(lua, p))
        .collect::<mlua::Result<Vec<_>>>()?;
    t.set("params", lua.create_sequence_from(params)?)?;
    t.set(
        "textures",
        lua.create_sequence_from(b.textures.iter().copied())?,
    )?;
    Ok(t)
}

fn param_table<'lua>(lua: &'lua Lua, p: &Param) -> mlua::Result<Table<'lua>> {
    let t = lua.create_table()?;
    t.set("name", p.name)?;
    t.set("arity", p.arity)?;
    t.set("default", default(lua, p)?)?;
    t.set("runtime", p.runtime)?;
    Ok(t)
}

/// A scalar for arity 1; a vector param's broadcast default as a ready-to-paste
/// array (`{0.5, 0.5, 0.5}`), the shape the recipe takes it in.
fn default<'lua>(lua: &'lua Lua, p: &Param) -> mlua::Result<Value<'lua>> {
    let d = f64::from(p.default);
    if p.arity == 1 {
        return Ok(Value::Number(d));
    }
    Ok(Value::Table(
        lua.create_sequence_from(std::iter::repeat_n(d, p.arity))?,
    ))
}
