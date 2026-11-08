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
//! Before the first `Harness.Step` there is no gameplay VM. `Move` then runs
//! the scenario's own, as before #1879: it moves the edit-mode controller and
//! calls no handler (there are none yet). `Damage.*` refuses: a hit with no
//! handler to hear it is a scenario's mistake.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Function, Lua, MultiValue, Result as LuaResult, Table};

use super::copy::Copier;
use crate::app::GameWorld;

/// The verbs forwarded, by namespace: each one whose callbacks run inside it,
/// and whether it falls back to the scenario's own while the game is down.
const FORWARDS: [(&str, &[&str], bool); 2] = [
    ("Damage", &["Apply", "ApplyPoint", "ApplyRadial"], false),
    ("CharacterController", &["Move"], true),
];

/// Replace the scenario's [`FORWARDS`] with forwards into the gameplay VM.
pub fn register(lua: &Lua, world: &Rc<RefCell<GameWorld>>) -> LuaResult<()> {
    for (namespace, verbs, falls_back) in FORWARDS {
        let table: Table = lua.globals().get(namespace)?;
        for &verb in verbs {
            let own = if falls_back {
                Some(table.get(verb)?)
            } else {
                None
            };
            table.set(verb, forward(lua, world, namespace, verb, own)?)?;
        }
    }
    Ok(())
}

/// `namespace.verb`, run in the gameplay VM; `own`, the scenario's, while
/// that VM is down (an error without it).
fn forward(
    lua: &Lua,
    world: &Rc<RefCell<GameWorld>>,
    namespace: &'static str,
    verb: &'static str,
    own: Option<Function>,
) -> LuaResult<Function> {
    let world = Rc::clone(world);
    lua.create_function(move |lua, args: MultiValue| {
        let ran = in_game(lua, &world.borrow(), namespace, verb, &args)?;
        match (ran, &own) {
            (Some(returns), _) => Ok(returns),
            (None, Some(own)) => own.call(args),
            (None, None) => Err(mlua::Error::RuntimeError(format!(
                "{namespace}.{verb}: the game's runtime is down"
            ))),
        }
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
