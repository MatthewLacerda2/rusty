//! src/api/texture/ops.rs — `Texture.Ops()`: the op catalog as Lua tables (#411).
//!
//! Straight from [`crate::procgen::recipe::OPS`], so what the agent reads is what
//! the parser accepts, which params take a link included. After the ops come the
//! project's graphs (#1744), each `.graph.json` under the project root as one
//! `op = "graph"` entry with its `path`, its inputs and its declared params.

use std::path::Path;

use mlua::{Lua, LuaSerdeExt, Table, Value};

use crate::procgen::graph::ParamDecl;
use crate::procgen::recipe::{Lit, OpInfo, OpParam, OPS};
use crate::procgen::TextureRecipe;
use crate::scene::authoring::material::{project_graphs, read_graph};

/// `{ {op, category, min_inputs, max_inputs, params = { {name, type, default, required, socket, values?} } }, … }`.
/// `socket` says whether the param takes a link to another node as well as a
/// number (#1494). The graphs under `root` follow, each with `path` and `inputs`
/// (the names it wires, in order) too, its params carrying `min` / `max` when
/// declared. A graph file that doesn't parse is left out, with a warning naming it.
pub fn ops_table(lua: &Lua, root: &Path) -> mlua::Result<Table> {
    let mut rows = OPS
        .iter()
        .map(|info| op_table(lua, info))
        .collect::<mlua::Result<Vec<_>>>()?;
    for path in project_graphs(root) {
        match read_graph(&path, root) {
            Ok(graph) => rows.push(graph_table(lua, &path, &graph)?),
            Err(e) => log::warn!("[Texture] Ops() leaves out graph {path:?}: {e}"),
        }
    }
    lua.create_sequence_from(rows)
}

fn graph_table(lua: &Lua, path: &str, graph: &TextureRecipe) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("op", "graph")?;
    t.set("path", path)?;
    t.set("category", "graph")?;
    t.set("min_inputs", graph.inputs.len())?;
    t.set("max_inputs", graph.inputs.len())?;
    t.set(
        "inputs",
        lua.create_sequence_from(graph.inputs.iter().map(String::as_str))?,
    )?;
    let params = graph
        .params
        .iter()
        .map(|(name, d)| graph_param(lua, name, d))
        .collect::<mlua::Result<Vec<_>>>()?;
    t.set("params", lua.create_sequence_from(params)?)?;
    Ok(t)
}

fn graph_param(lua: &Lua, name: &str, d: &ParamDecl) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("name", name)?;
    t.set("type", lua.to_value(&d.ty)?)?;
    t.set("default", lua.to_value(&d.default)?)?;
    t.set("required", false)?;
    t.set("socket", false)?;
    t.set("min", d.min)?;
    t.set("max", d.max)?;
    Ok(t)
}

fn op_table(lua: &Lua, info: &OpInfo) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("op", info.op)?;
    t.set("category", info.category)?;
    t.set("min_inputs", info.inputs.min)?;
    t.set("max_inputs", info.inputs.max)?;
    let params = info
        .params
        .iter()
        .map(|p| param_table(lua, p))
        .collect::<mlua::Result<Vec<_>>>()?;
    t.set("params", lua.create_sequence_from(params)?)?;
    Ok(t)
}

fn param_table(lua: &Lua, p: &OpParam) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("name", p.name)?;
    t.set("type", p.ty)?;
    t.set("required", p.default.is_none())?;
    t.set("socket", p.socket)?;
    if let Some(d) = p.default {
        t.set("default", lit(lua, d)?)?;
    }
    if !p.values.is_empty() {
        t.set(
            "values",
            lua.create_sequence_from(p.values.iter().copied())?,
        )?;
    }
    Ok(t)
}

fn lit(lua: &Lua, l: Lit) -> mlua::Result<Value> {
    Ok(match l {
        Lit::Number(n) => Value::Number(f64::from(n)),
        Lit::Integer(i) => Value::Integer(i64::from(i)),
        Lit::Bool(b) => Value::Boolean(b),
        Lit::Vec2(v) => Value::Table(lua.create_sequence_from(v)?),
        Lit::Text(s) => Value::String(lua.create_string(s)?),
    })
}

#[cfg(test)]
mod tests {
    use mlua::Lua;

    #[test]
    fn ops_reports_which_params_take_a_link() {
        let lua = Lua::new();
        super::super::register(&lua).unwrap();
        let (warp, scale): (bool, bool) = lua
            .load(
                r#"local socket = {}
                   for _, o in ipairs(Texture.Ops()) do
                     for _, p in ipairs(o.params) do socket[o.op .. "." .. p.name] = p.socket end
                   end
                   return socket["warp.strength"], socket["noise.scale"]"#,
            )
            .eval()
            .unwrap();
        assert_eq!((warp, scale), (true, false));
    }

    #[test]
    fn ops_describes_every_op_with_typed_params_and_defaults() {
        let lua = Lua::new();
        super::super::register(&lua).unwrap();
        let (count, category, inputs, octaves, required, kinds, mix_min, mix_max): (
            usize,
            String,
            u8,
            u32,
            bool,
            String,
            u8,
            u8,
        ) = lua
            .load(
                r#"local ops, by_op = Texture.Ops(), {}
                   for _, o in ipairs(ops) do by_op[o.op] = o end
                   local o, mix = by_op.noise, by_op.mix
                   local by = {}
                   for _, p in ipairs(o.params) do by[p.name] = p end
                   return #ops, o.category, o.max_inputs, by.octaves.default,
                     by.scale.required, table.concat(by.kind.values, "|"),
                     mix.min_inputs, mix.max_inputs"#,
            )
            .eval()
            .unwrap();
        assert_eq!(count, crate::procgen::recipe::OPS.len());
        assert_eq!((category.as_str(), inputs), ("generator", 0));
        assert_eq!(
            (octaves, required, kinds.as_str()),
            (1, true, "perlin|fbm|ridged|turbulence")
        );
        assert_eq!(
            (mix_min, mix_max),
            (2, 3),
            "min_inputs and max_inputs both reach Lua"
        );
    }
}
