//! src/api/texture/ops.rs — `Texture.Ops()`: the op catalog as Lua tables (#411).
//!
//! Straight from [`crate::procgen::recipe::OPS`], so what the agent reads is what
//! the parser accepts.

use mlua::{Lua, Table, Value};

use crate::procgen::recipe::{Lit, OpInfo, OpParam, OPS};

/// `{ {op, category, min_inputs, max_inputs, params = { {name, type, default, required, values?} } }, … }`.
pub fn ops_table(lua: &Lua) -> mlua::Result<Table> {
    lua.create_sequence_from(
        OPS.iter()
            .map(|info| op_table(lua, info))
            .collect::<mlua::Result<Vec<_>>>()?,
    )
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
        Lit::Text(s) => Value::String(lua.create_string(s)?),
    })
}

#[cfg(test)]
mod tests {
    use mlua::Lua;

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
