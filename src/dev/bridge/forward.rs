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

use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Function, Lua, MultiValue, Result as LuaResult, Table};

use super::copy::Copier;
use crate::app::GameWorld;

/// The verbs forwarded, by namespace: each one whose callbacks run inside it.
const FORWARDS: [(&str, &[&str]); 2] = [
    ("Damage", &["Apply", "ApplyPoint", "ApplyRadial"]),
    ("CharacterController", &["Move"]),
];

/// Replace the scenario's [`FORWARDS`] with forwards into the gameplay VM.
pub fn register(lua: &Lua, world: &Rc<RefCell<GameWorld>>) -> LuaResult<()> {
    for (namespace, verbs) in FORWARDS {
        let table: Table = lua.globals().get(namespace)?;
        for &verb in verbs {
            table.set(verb, forward(lua, world, namespace, verb)?)?;
        }
    }
    Ok(())
}

/// `namespace.verb`, run in the gameplay VM.
fn forward(
    lua: &Lua,
    world: &Rc<RefCell<GameWorld>>,
    namespace: &'static str,
    verb: &'static str,
) -> LuaResult<Function> {
    let world = Rc::clone(world);
    lua.create_function(move |lua, args: MultiValue| {
        let world = world.borrow();
        let ran: Option<MultiValue> = world.script_manager().with_vm(|glua| {
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
        })?;
        ran.ok_or_else(|| {
            mlua::Error::RuntimeError(format!("{namespace}.{verb}: the game's runtime is down"))
        })
    })
}
