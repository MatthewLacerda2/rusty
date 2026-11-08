//! The scenario's calls that must run in the gameplay VM: `Damage.*` (#1893)
//! and `CharacterController.Move` (#1879).
//!
//! Both call script callbacks synchronously, inside the call (`OnTakeDamage`,
//! `OnControllerColliderHit`), on the live script instances, and those live in
//! the gameplay VM: the scenario's own copy of the namespace would find none.
//! So each verb here copies its arguments into the gameplay VM (`copy`), calls
//! that VM's `<Namespace>.<verb>` and copies the returns back, the way a
//! `Scene.GetScript` handle calls a game function (`script_ref`). A handler's
//! error is raised as the scenario's.
//!
//! The verbs, and what each does before the first `Harness.Step` (no gameplay
//! VM yet), are `demo::forwards`' table: the replay makes them by the same
//! rule, so a demo recorded through these forwards replays (#1929).

use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Function, Lua, MultiValue, Result as LuaResult, Table};

use super::copy::Copier;
use crate::app::GameWorld;
use crate::dev::demo::{Forward, FORWARDS};

/// Replace the scenario's [`FORWARDS`] with forwards into the gameplay VM.
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

/// `entry`'s `verb`, run in the gameplay VM, or as [`Forward::run`] says while
/// that VM is down (`own`, the scenario's, for a verb that falls back).
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
/// `namespace.verb(args)` in the gameplay VM, its returns copied back to
/// `lua`; `None` while that VM is down.
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
