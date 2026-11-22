use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Function, Lua, MultiValue, Result as LuaResult, Table};

use super::copy::Copier;
use crate::demo::{Forward, FORWARDS};
use rusty_app::GameWorld;

pub fn register(lua: &Lua, world: &Rc<RefCell<GameWorld>>) -> LuaResult<()> {
    for entry in &FORWARDS {
        let table: Table = lua.globals().get(entry.namespace)?;
        for &verb in entry.verbs {
            let own: Function = table.get(verb)?;
            table.set(verb, forward(lua, world, entry, verb, own)?)?;
        }
    }
    Ok(())
}

fn forward(
    lua: &Lua,
    world: &Rc<RefCell<GameWorld>>,
    entry: &'static Forward,
    verb: &'static str,
    own: Function,
) -> LuaResult<Function> {
    let world = Rc::clone(world);
    lua.create_function(move |lua, args: MultiValue| {
        entry.run(
            verb,
            || in_game(lua, &world.borrow(), entry.namespace, verb, &args),
            || own.call(args.clone()),
        )
    })
}
fn in_game(
    lua: &Lua,
    world: &GameWorld,
    namespace: &str,
    verb: &str,
    args: &MultiValue,
) -> LuaResult<Option<MultiValue>> {
    world.script_manager().with_vm(|glua| {
        let table: Table = glua.globals().get(namespace)?;
        let f: Function = table.get(verb)?;
        let mut into_game = Copier::new(glua);
        let crossed = args
            .iter()
            .map(|v| into_game.value(v))
            .collect::<LuaResult<MultiValue>>()?;
        let returns = f.call::<MultiValue>(crossed)?;
        let mut back = Copier::new(lua);
        returns.iter().map(|v| back.value(v)).collect()
    })
}
