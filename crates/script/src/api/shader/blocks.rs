use mlua::{Lua, Table, Value};

use rusty_shadergen::blocks::{catalog, Block, Param, Stage};
use rusty_shadergen::recipe::PassKind;

pub fn blocks_table(lua: &Lua, pass: PassKind) -> mlua::Result<Table> {
    lua.create_sequence_from(
        catalog(pass)
            .iter()
            .map(|b| block_table(lua, b))
            .collect::<mlua::Result<Vec<_>>>()?,
    )
}

fn block_table(lua: &Lua, b: &Block) -> mlua::Result<Table> {
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
    let stage = match b.stage {
        Stage::Color => "color",
        Stage::Uv => "uv",
        Stage::Vertex => "vertex",
    };
    t.set("stage", stage)?;
    Ok(t)
}

fn param_table(lua: &Lua, p: &Param) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("name", p.name)?;
    t.set("arity", p.arity)?;
    t.set("default", default(lua, p)?)?;
    t.set("runtime", p.runtime)?;
    if !p.choices.is_empty() {
        t.set(
            "choices",
            lua.create_sequence_from(p.choices.iter().copied())?,
        )?;
    }
    Ok(t)
}

fn default(lua: &Lua, p: &Param) -> mlua::Result<Value> {
    if let Some(first) = p.choices.first() {
        return Ok(Value::String(lua.create_string(first)?));
    }
    if p.arity == 1 {
        return Ok(Value::Number(f64::from(p.default)));
    }
    let lanes = p.defaults().into_iter().map(f64::from);
    Ok(Value::Table(lua.create_sequence_from(lanes)?))
}
