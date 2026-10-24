//! src/dev/bridge/script_ref.rs — `Scene.GetScript` across the VM boundary (#1384).
//!
//! The scenario VM registers the real API, whose `Scene.GetScript` looks the
//! instance up in the VM that calls it, and a script instance lives in the
//! *gameplay* VM: a Lua table never leaves the VM that made it. So the scenario
//! VM's `GetScript` is this file's: it answers a [`ScriptRef`], a live handle that
//! reads and writes the instance's fields by value, copied Lua-to-Lua (`copy`).
//! Fields are read and written raw, so no gameplay metamethod runs in the
//! scenario's call. A function field cannot cross and reads as an error; call
//! gameplay code by stepping the world.
//!
//! The handle pins the one instance it was made for: once that instance is gone
//! (its entity destroyed, Play stopped, the script reloaded), every access errors,
//! even when a new instance has taken the same entity id and script name.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::{
    Lua, MetaMethod, RegistryKey, Result as LuaResult, Table, UserData, UserDataMethods, Value,
};

use super::copy::copy;
use crate::app::GameWorld;

/// A scenario's handle on one live script instance (see the module doc).
struct ScriptRef {
    world: Rc<RefCell<GameWorld>>,
    id: u32,
    name: String,
    /// The instance's table, held in the gameplay VM's registry: its identity.
    pinned: RegistryKey,
}

impl ScriptRef {
    /// Run `f` on the pinned instance in the gameplay VM, or fail when it is no
    /// longer the entity's live script of that name.
    fn with<R>(&self, f: impl FnOnce(&Lua, &Table) -> LuaResult<R>) -> LuaResult<R> {
        let world = self.world.borrow();
        let live = world
            .script_manager()
            .with_script(self.id, &self.name, |glua, t| {
                let same = glua
                    .registry_value::<Table>(&self.pinned)
                    .is_ok_and(|pinned| pinned.to_pointer() == t.to_pointer());
                same.then(|| f(glua, t)).transpose()
            })?;
        live.flatten().ok_or_else(|| {
            mlua::Error::RuntimeError(format!(
                "script '{}' on entity {} is gone",
                self.name, self.id
            ))
        })
    }
}

impl UserData for ScriptRef {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::Index, |lua, this, key: Value| {
            this.with(|glua, t| copy(&t.raw_get::<Value>(copy(&key, glua)?)?, lua))
        });
        methods.add_meta_method(
            MetaMethod::NewIndex,
            |_, this, (key, value): (Value, Value)| {
                this.with(|glua, t| t.raw_set(copy(&key, glua)?, copy(&value, glua)?))
            },
        );
    }
}

/// Replace the scenario VM's `Scene.GetScript` with one that answers a
/// [`ScriptRef`], or `nil` when the entity has no live script of that name.
pub fn register(lua: &Lua, world: &Rc<RefCell<GameWorld>>) -> LuaResult<()> {
    let world = Rc::clone(world);
    let get = lua.create_function(move |lua, (id, name): (u32, String)| {
        let pinned = world
            .borrow()
            .script_manager()
            .with_script(id, &name, |glua, t| glua.create_registry_value(t.clone()))?;
        let Some(pinned) = pinned else {
            return Ok(Value::Nil);
        };
        let handle = ScriptRef {
            world: Rc::clone(&world),
            id,
            name,
            pinned,
        };
        lua.create_userdata(handle).map(Value::UserData)
    })?;
    lua.globals()
        .get::<mlua::Table>("Scene")?
        .set("GetScript", get)
}
