//! A plain Lua value copied from one VM into another (#1384).
//!
//! A Lua value never leaves the VM that made it, so a field crossing between the
//! scenario VM and the gameplay VM (`script_ref`) is rebuilt on the other side:
//! nil, booleans, integers, floats (`math.huge` and NaN too) and strings as they
//! are, tables recursively, keys included, so `{[0]=true}` keeps its integer key
//! and `{"rifle", ammo=30}` stays one mixed table. A table met twice is copied
//! once (shared and cyclic tables keep their shape). Metatables are not copied;
//! a function, userdata or thread cannot cross and is an error, and so is a table
//! nested deeper than [`MAX_DEPTH`] (an error, not a stack overflow).

use std::collections::BTreeMap;

use mlua::{Lua, Result as LuaResult, Table, Value};

/// How deep a copied table may nest. Lua's own C-call limit is 200, so no data a
/// script means to pass comes near it; a deeper chain is refused before the
/// recursion can overflow the native stack.
pub const MAX_DEPTH: usize = 200;

/// `value`, rebuilt in `into` (see the module doc).
pub fn copy(value: &Value, into: &Lua) -> LuaResult<Value> {
    Copier {
        into,
        seen: BTreeMap::new(),
        depth: 0,
    }
    .value(value)
}

struct Copier<'a> {
    into: &'a Lua,
    /// Source tables already copied, by address, so each is copied once.
    seen: BTreeMap<usize, Table>,
    /// Tables entered and not yet left, against [`MAX_DEPTH`].
    depth: usize,
}

impl Copier<'_> {
    fn value(&mut self, value: &Value) -> LuaResult<Value> {
        Ok(match value {
            Value::Nil => Value::Nil,
            Value::Boolean(b) => Value::Boolean(*b),
            Value::Integer(i) => Value::Integer(*i),
            Value::Number(n) => Value::Number(*n),
            Value::String(s) => Value::String(self.into.create_string(&*s.as_bytes())?),
            Value::Table(t) => Value::Table(self.table(t)?),
            other => {
                return Err(mlua::Error::RuntimeError(format!(
                    "a {} cannot cross between the scenario VM and the gameplay VM; \
                     call gameplay code by stepping the world",
                    other.type_name()
                )))
            }
        })
    }

    fn table(&mut self, source: &Table) -> LuaResult<Table> {
        let address = source.to_pointer() as usize;
        if let Some(copied) = self.seen.get(&address) {
            return Ok(copied.clone());
        }
        if self.depth == MAX_DEPTH {
            return Err(mlua::Error::RuntimeError(format!(
                "a table nested deeper than {MAX_DEPTH} cannot cross between the scenario VM \
                 and the gameplay VM"
            )));
        }
        let copied = self.into.create_table()?;
        self.seen.insert(address, copied.clone());
        self.depth += 1;
        for pair in source.pairs::<Value, Value>() {
            let (key, value) = pair?;
            copied.raw_set(self.value(&key)?, self.value(&value)?)?;
        }
        self.depth -= 1;
        Ok(copied)
    }
}
