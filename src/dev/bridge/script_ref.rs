//! src/dev/bridge/script_ref.rs — `Scene.GetScript` across the VM boundary (#1384).
//!
//! The scenario VM registers the real API, whose `Scene.GetScript` looks the
//! instance up in the VM that calls it, and a script instance lives in the
//! *gameplay* VM: a Lua table never leaves the VM that made it. So the scenario
//! VM's `GetScript` is this file's: it answers a [`ScriptRef`], a live handle that
//! reads and writes the instance's fields by value, copied Lua-to-Lua (`copy`).
//! Fields are read and written raw, so no gameplay metamethod runs in the
//! scenario's call.
//!
//! A function field reads as a proxy that runs the game script's function in the
//! gameplay VM (#1580), the way Unity's test runner calls a MonoBehaviour's public
//! method: `ref.Refill(30)` with plain arguments, `ref:Refill(30)` with the
//! instance itself as `self`. It runs at the current tick, between the
//! scenario's steps, inside an API scope like a lifecycle hook's; arguments and
//! returns are copied, so a function or userdata on either side is an error (a
//! return that cannot cross fails after the call ran: its side effects stand), and a
//! game script error comes back as the scenario's, with its traceback. The call is
//! part of the scenario's own script, so a run stays a pure function of (seed,
//! scenario).
//!
//! The handle pins the one instance it was made for: once that instance is gone
//! (its entity destroyed, Play stopped, the script reloaded), every access errors,
//! even when a new instance has taken the same entity id and script name. A
//! proxy pins its function the same way: it calls the function it was read as.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::{
    AnyUserData, Function, Lua, MetaMethod, MultiValue, RegistryKey, Result as LuaResult, Table,
    UserData, UserDataMethods, Value,
};

use super::copy::{copy, Copier};
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
    /// longer the entity's live script of that name. `scoped` registers the API
    /// first, for a call into the game script's code.
    fn with<R>(&self, scoped: bool, f: impl FnOnce(&Lua, &Table) -> LuaResult<R>) -> LuaResult<R> {
        let world = self.world.borrow();
        let manager = world.script_manager();
        let on_pinned = |glua: &Lua, t: &Table| {
            let same = glua
                .registry_value::<Table>(&self.pinned)
                .is_ok_and(|pinned| pinned.to_pointer() == t.to_pointer());
            same.then(|| f(glua, t)).transpose()
        };
        let live = if scoped {
            manager.with_script_in_api_scope(self.id, &self.name, on_pinned)?
        } else {
            manager.with_script(self.id, &self.name, on_pinned)?
        };
        live.flatten().ok_or_else(|| {
            mlua::Error::RuntimeError(format!(
                "script '{}' on entity {} is gone",
                self.name, self.id
            ))
        })
    }

    /// Call the gameplay function `func` (named `key`) with the scenario's `args`:
    /// a first argument that is `handle` itself becomes the instance (`self`).
    fn call(
        &self,
        handle: &AnyUserData,
        func: &RegistryKey,
        key: &str,
        args: MultiValue,
        lua: &Lua,
    ) -> LuaResult<MultiValue> {
        let context = |what: String| {
            mlua::Error::RuntimeError(format!(
                "script '{}' on entity {}: {key} {what}",
                self.name, self.id
            ))
        };
        self.with(true, |glua, t| {
            let func: Function = glua.registry_value(func)?;
            let mut into_game = Copier::new(glua);
            let mut crossed = MultiValue::new();
            for (i, arg) in args.iter().enumerate() {
                let is_self = i == 0
                    && matches!(arg, Value::UserData(u) if u.to_pointer() == handle.to_pointer());
                crossed.push_back(if is_self {
                    Value::Table(t.clone())
                } else {
                    into_game.value(arg)?
                });
            }
            let returns = func
                .call::<MultiValue>(crossed)
                .map_err(|e| context(format!("failed: {e}")))?;
            let mut into_scenario = Copier::new(lua);
            returns
                .iter()
                .map(|v| into_scenario.value(v))
                .collect::<LuaResult<_>>()
                .map_err(|e| {
                    context(format!(
                        "returned a value that cannot cross back; the call itself ran ({e})"
                    ))
                })
        })
    }
}

/// `field` of the pinned instance as the scenario sees it: a copy of its value,
/// or for a function, a proxy that calls it (see the module doc).
fn index(lua: &Lua, handle: AnyUserData, key: Value) -> LuaResult<Value> {
    let this = handle.borrow::<ScriptRef>()?;
    let pinned_fn = this.with(false, |glua, t| {
        match t.raw_get::<Value>(copy(&key, glua)?)? {
            Value::Function(f) => glua.create_registry_value(f).map(Err),
            other => copy(&other, lua).map(Ok),
        }
    })?;
    let func = match pinned_fn {
        Ok(value) => return Ok(value),
        Err(func) => func,
    };
    let name = key.to_string()?;
    drop(this);
    let proxy = lua.create_function(move |lua, args: MultiValue| {
        handle
            .borrow::<ScriptRef>()?
            .call(&handle, &func, &name, args, lua)
    })?;
    Ok(Value::Function(proxy))
}

impl UserData for ScriptRef {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_function(MetaMethod::Index, |lua, (handle, key)| {
            index(lua, handle, key)
        });
        methods.add_meta_method(
            MetaMethod::NewIndex,
            |_, this, (key, value): (Value, Value)| {
                this.with(false, |glua, t| {
                    t.raw_set(copy(&key, glua)?, copy(&value, glua)?)
                })
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
