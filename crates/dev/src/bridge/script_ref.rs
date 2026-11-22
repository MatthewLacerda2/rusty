use std::cell::RefCell;
use std::rc::Rc;

use mlua::{
    AnyUserData, Function, Lua, MetaMethod, MultiValue, RegistryKey, Result as LuaResult, Table,
    UserData, UserDataMethods, Value,
};

use super::copy::{copy, Copier};
use crate::demo;
use rusty_app::GameWorld;
use rusty_script::scripting::HostJournal;

struct ScriptRef {
    world: Rc<RefCell<GameWorld>>,
    id: u32,
    name: String,
    pinned: RegistryKey,
    journal: Rc<RefCell<HostJournal>>,
}

impl ScriptRef {
    fn with<R>(&self, f: impl FnOnce(&Lua, &Table) -> LuaResult<R>) -> LuaResult<R> {
        let world = self.world.borrow();
        let manager = world.script_manager();
        let on_pinned = |glua: &Lua, t: &Table| {
            let same = glua
                .registry_value::<Table>(&self.pinned)
                .is_ok_and(|pinned| pinned.to_pointer() == t.to_pointer());
            same.then(|| f(glua, t)).transpose()
        };
        let live = manager.with_script(self.id, &self.name, on_pinned)?;
        live.flatten().ok_or_else(|| {
            mlua::Error::RuntimeError(format!(
                "script '{}' on entity {} is gone",
                self.name, self.id
            ))
        })
    }

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
        let with_self = matches!(args.front(), Some(Value::UserData(u)) if u.to_pointer() == handle.to_pointer());
        let noted: Vec<Value> = args.iter().skip(usize::from(with_self)).cloned().collect();
        let script = (self.id, self.name.as_str());
        self.with(|glua, t| {
            demo::note_script_call(&self.journal, script, (key, with_self), &noted);
            let func: Function = glua.registry_value(func)?;
            let mut into_game = Copier::new(glua);
            let mut crossed = MultiValue::new();
            for (i, arg) in args.iter().enumerate() {
                let is_self = i == 0 && with_self;
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

fn index(lua: &Lua, handle: AnyUserData, key: Value) -> LuaResult<Value> {
    let this = handle.borrow::<ScriptRef>()?;
    let pinned_fn = this.with(|glua, t| match t.raw_get::<Value>(copy(&key, glua)?)? {
        Value::Function(f) => glua.create_registry_value(f).map(Err),
        other => copy(&other, lua).map(Ok),
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
                this.with(|glua, t| t.raw_set(copy(&key, glua)?, copy(&value, glua)?))?;
                let script = (this.id, this.name.as_str());
                demo::note_script_set(&this.journal, script, &key, &value);
                Ok(())
            },
        );
    }
}

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
        let journal = world.borrow().script_manager().journal();
        let handle = ScriptRef {
            world: Rc::clone(&world),
            id,
            name,
            pinned,
            journal,
        };
        lua.create_userdata(handle).map(Value::UserData)
    })?;
    lua.globals()
        .get::<mlua::Table>("Scene")?
        .set("GetScript", get)
}
