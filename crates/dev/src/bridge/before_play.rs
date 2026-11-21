use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Function, Lua, MultiValue, Result as LuaResult, Table};

use rusty_app::GameWorld;

const WIPED_BY_PLAY: &[(&str, &[&str])] = &[
    ("Timer", &["Invoke", "InvokeRepeating", "StartCoroutine"]),
    ("Tween", &["To", "Sequence"]),
    ("Random", &["SetSeed"]),
];

pub fn register(lua: &Lua, world: &Rc<RefCell<GameWorld>>) -> LuaResult<()> {
    for &(namespace, verbs) in WIPED_BY_PLAY {
        let table: Table = lua.globals().get(namespace)?;
        for &verb in verbs {
            let inner: Function = table.get(verb)?;
            let world = Rc::clone(world);
            let name = format!("{namespace}.{verb}");
            let guarded = lua.create_function(move |_, args: MultiValue| {
                if !world.borrow().scene().borrow().is_playing() {
                    return Err(mlua::Error::RuntimeError(format!(
                        "{name} before the first Harness.Step: entering Play on that step \
                         resets timers, tweens and the Random seed, so this would be lost; \
                         call Harness.Step(1) first"
                    )));
                }
                inner.call::<MultiValue>(args)
            })?;
            table.set(verb, guarded)?;
        }
    }
    Ok(())
}
