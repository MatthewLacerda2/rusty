use mlua::{Lua, Table, Value};

use crate::api::lua_json::to_lua;
use rusty_core::project::ContentRoots;
use rusty_edit::authoring::material::{listed_graphs, read_graph};
use rusty_procgen::graph::ParamDecl;
use rusty_procgen::recipe::{Lit, OpInfo, OpParam, OPS};
use rusty_procgen::TextureRecipe;

pub fn ops_table(lua: &Lua, roots: &ContentRoots) -> mlua::Result<Table> {
    let mut rows = OPS
        .iter()
        .map(|info| op_table(lua, info))
        .collect::<mlua::Result<Vec<_>>>()?;
    for path in listed_graphs(roots) {
        match read_graph(&path, roots) {
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
    t.set("type", to_lua(lua, &d.ty)?)?;
    t.set("default", to_lua(lua, &d.default)?)?;
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
    use rusty_core::project::ContentRoots;

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
                r#"local ops, by_op, count = Texture.Ops(), {}, 0
                   for _, o in ipairs(ops) do
                     by_op[o.op] = o
                     -- the engine's graphs follow the ops: count the ops alone
                     if o.op ~= "graph" then count = count + 1 end
                   end
                   local o, mix = by_op.noise, by_op.mix
                   local by = {}
                   for _, p in ipairs(o.params) do by[p.name] = p end
                   return count, o.category, o.max_inputs, by.octaves.default,
                     by.scale.required, table.concat(by.kind.values, "|"),
                     mix.min_inputs, mix.max_inputs"#,
            )
            .eval()
            .unwrap();
        assert_eq!(count, rusty_procgen::recipe::OPS.len());
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

    #[test]
    fn ops_lists_the_projects_graphs_with_their_interface() {
        let base = rusty_test_support::temp::dir().join("rusty_texture_ops_graphs_1744");
        let root = base.join("game");
        // An empty engine folder, or the engine's own graphs would follow the project's.
        let roots = ContentRoots::with_engine(&root, &base.join("engine"));
        std::fs::create_dir_all(root.join("textures")).unwrap();
        let graph = serde_json::json!({
            "resolution": 8, "inputs": ["mask"],
            "params": { "age": { "type": "number", "default": 0.5, "min": 0, "max": 1 } },
            "nodes": [{ "id": "i", "op": "invert", "inputs": ["mask"] }]
        });
        let file = root.join("textures").join("wear.graph.json");
        std::fs::write(file, graph.to_string()).unwrap();
        std::fs::write(root.join("broken.graph.json"), "{").unwrap();
        let lua = Lua::new();
        let ops = super::ops_table(&lua, &roots).unwrap();
        lua.globals().set("OPS", ops).unwrap();
        let (count, path, inputs, name, max): (usize, String, u8, String, f64) = lua
            .load(
                r#"local g = OPS[#OPS]
                   return #OPS, g.path, g.max_inputs, g.inputs[1] .. "/" .. g.params[1].name,
                     g.params[1].max"#,
            )
            .eval()
            .unwrap();
        assert_eq!(
            count,
            rusty_procgen::recipe::OPS.len() + 1,
            "broken one left out"
        );
        assert_eq!(path, "textures/wear.graph.json");
        assert_eq!((inputs, name.as_str(), max), (1, "mask/age", 1.0));
    }

    #[test]
    fn ops_lists_the_engines_graphs_by_their_engine_path() {
        let base = rusty_test_support::temp::dir().join("rusty_texture_ops_engine_2185");
        let roots = ContentRoots::with_engine(&base.join("game"), &base.join("engine"));
        let library = roots.engine.join("textures").join("library");
        std::fs::create_dir_all(&library).unwrap();
        std::fs::create_dir_all(&roots.project).unwrap();
        let graph = serde_json::json!({
            "resolution": 8, "inputs": ["mask"],
            "nodes": [{ "id": "i", "op": "invert", "inputs": ["mask"] }]
        });
        std::fs::write(library.join("rust.graph.json"), graph.to_string()).unwrap();
        let lua = Lua::new();
        let ops = super::ops_table(&lua, &roots).unwrap();
        lua.globals().set("OPS", ops).unwrap();
        let (op, path, input): (String, String, String) = lua
            .load(r#"local g = OPS[#OPS] return g.op, g.path, g.inputs[1]"#)
            .eval()
            .unwrap();
        let want = ("graph", "engine/textures/library/rust.graph.json", "mask");
        assert_eq!((op.as_str(), path.as_str(), input.as_str()), want);
    }
}
