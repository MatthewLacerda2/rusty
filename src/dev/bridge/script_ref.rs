//! src/dev/bridge/script_ref.rs — `Scene.GetScript` across the VM boundary (#1384).
//!
//! The scenario VM registers the real API, whose `Scene.GetScript` looks the
//! instance up in the VM that calls it, and a script instance lives in the
//! *gameplay* VM: a Lua table never leaves the VM that made it. So the scenario
//! VM's `GetScript` is this file's: it answers a [`ScriptRef`], a live handle that
//! reads and writes the instance's fields by value, through the same serde bridge
//! (`api::lua_json`) a recipe or a `Storage` value crosses. Fields are read and
//! written raw, so no gameplay metamethod runs in the scenario's call. A function
//! field cannot cross and reads as an error; call gameplay code by stepping the world.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Lua, MetaMethod, Result as LuaResult, UserData, UserDataMethods, Value};

use crate::api::lua_json::{lua_to_json, to_lua};
use crate::app::GameWorld;

/// A scenario's handle on entity `id`'s live script `name` (see the module doc).
struct ScriptRef {
    world: Rc<RefCell<GameWorld>>,
    id: u32,
    name: String,
}

impl ScriptRef {
    /// Run `f` on the instance in the gameplay VM, or fail when it is gone (the
    /// entity was destroyed, or Play stopped and took the VM with it).
    fn with<R>(&self, f: impl FnOnce(&Lua, &mlua::Table) -> LuaResult<R>) -> LuaResult<R> {
        let world = self.world.borrow();
        world
            .script_manager()
            .with_script(self.id, &self.name, f)?
            .ok_or_else(|| {
                mlua::Error::RuntimeError(format!(
                    "script '{}' on entity {} is gone",
                    self.name, self.id
                ))
            })
    }
}

/// A field key that means the same in both VMs: a string or an integer.
enum Key {
    Name(String),
    Index(i64),
}

impl Key {
    fn of(value: &Value) -> LuaResult<Key> {
        match value {
            Value::String(s) => Ok(Key::Name(s.to_str()?.to_string())),
            Value::Integer(i) => Ok(Key::Index(*i)),
            other => Err(mlua::Error::RuntimeError(format!(
                "a script field key is a string or an integer, got {}",
                other.type_name()
            ))),
        }
    }

    fn raw_get(&self, table: &mlua::Table) -> LuaResult<Value> {
        match self {
            Key::Name(k) => table.raw_get(k.as_str()),
            Key::Index(i) => table.raw_get(*i),
        }
    }

    fn raw_set(&self, table: &mlua::Table, value: Value) -> LuaResult<()> {
        match self {
            Key::Name(k) => table.raw_set(k.as_str(), value),
            Key::Index(i) => table.raw_set(*i, value),
        }
    }
}

/// A Lua value from one VM as plain data, ready for the other.
fn to_json(value: &Value) -> LuaResult<serde_json::Value> {
    lua_to_json(value).map_err(mlua::Error::RuntimeError)
}

impl UserData for ScriptRef {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::Index, |lua, this, key: Value| {
            let key = Key::of(&key)?;
            let json = this.with(|_, t| to_json(&key.raw_get(t)?))?;
            to_lua(lua, &json)
        });
        methods.add_meta_method(
            MetaMethod::NewIndex,
            |_, this, (key, value): (Value, Value)| {
                let key = Key::of(&key)?;
                let json = to_json(&value)?;
                this.with(|glua, t| key.raw_set(t, to_lua(glua, &json)?))
            },
        );
    }
}

/// Replace the scenario VM's `Scene.GetScript` with one that answers a
/// [`ScriptRef`], or `nil` when the entity has no live script of that name.
pub fn register(lua: &Lua, world: &Rc<RefCell<GameWorld>>) -> LuaResult<()> {
    let world = Rc::clone(world);
    let get = lua.create_function(move |lua, (id, name): (u32, String)| {
        let live = world
            .borrow()
            .script_manager()
            .with_script(id, &name, |_, _| Ok(()))?
            .is_some();
        if !live {
            return Ok(Value::Nil);
        }
        let handle = ScriptRef {
            world: Rc::clone(&world),
            id,
            name,
        };
        lua.create_userdata(handle).map(Value::UserData)
    })?;
    lua.globals()
        .get::<mlua::Table>("Scene")?
        .set("GetScript", get)
}
