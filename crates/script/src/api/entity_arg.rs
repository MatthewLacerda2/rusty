use mlua::{FromLua, Lua, Value};

use rusty_ecs::NO_ENTITY;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntityArg(Option<u32>);

impl EntityArg {
    pub fn read(self) -> u32 {
        self.0.unwrap_or(NO_ENTITY)
    }

    pub fn live(self, namespace: &str, verb: &str) -> mlua::Result<u32> {
        self.0.ok_or_else(|| {
            mlua::Error::RuntimeError(format!("{namespace}.{verb}: no entity (nil id)"))
        })
    }

    pub fn get(self) -> Option<u32> {
        self.0
    }
}

impl From<u32> for EntityArg {
    fn from(id: u32) -> Self {
        Self(Some(id))
    }
}

impl FromLua for EntityArg {
    fn from_lua(value: Value, lua: &Lua) -> mlua::Result<Self> {
        match value {
            Value::Nil => Ok(Self(None)),
            other => u32::from_lua(other, lua).map(|id| Self(Some(id))),
        }
    }
}

#[cfg(test)]
#[path = "entity_arg_tests.rs"]
mod tests;
