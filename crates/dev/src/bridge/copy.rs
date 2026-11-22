use std::collections::BTreeMap;

use mlua::{Lua, Result as LuaResult, Table, Value};

/// Tables a copy may nest: Lua 5.4's own C-call limit is 200, so a deeper chain is
/// refused before the recursion can overflow the native stack.
pub const MAX_DEPTH: usize = 200;

pub fn copy(value: &Value, into: &Lua) -> LuaResult<Value> {
    Copier::new(into).value(value)
}

pub struct Copier<'a> {
    into: &'a Lua,
    seen: BTreeMap<usize, Table>,
    depth: usize,
}

impl<'a> Copier<'a> {
    pub fn new(into: &'a Lua) -> Self {
        Self {
            into,
            seen: BTreeMap::new(),
            depth: 0,
        }
    }

    pub fn value(&mut self, value: &Value) -> LuaResult<Value> {
        Ok(match value {
            Value::Nil => Value::Nil,
            Value::Boolean(b) => Value::Boolean(*b),
            Value::Integer(i) => Value::Integer(*i),
            Value::Number(n) => Value::Number(*n),
            Value::String(s) => Value::String(self.into.create_string(&*s.as_bytes())?),
            Value::Table(t) => Value::Table(self.table(t)?),
            other => {
                return Err(mlua::Error::RuntimeError(format!(
                    "a {} (or a table holding one) cannot cross between the scenario VM and \
                     the gameplay VM: only plain data does (numbers, strings, booleans, \
                     tables of those, entity ids)",
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
