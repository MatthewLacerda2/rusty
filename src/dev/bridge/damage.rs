//! A scenario's `Damage.*` (#1893), run in the gameplay VM.
//!
//! `Damage` delivers synchronously to the `OnTakeDamage` handlers of the live
//! script instances, and those live in the gameplay VM: the scenario's own copy of
//! the namespace would find none. So each verb here copies its arguments into the
//! gameplay VM (`copy`), calls that VM's `Damage.<verb>` and copies the returns
//! back, the way a `Scene.GetScript` handle calls a game function (`script_ref`).
//! A handler's error is raised as the scenario's.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Function, Lua, MultiValue, Result as LuaResult, Table};

use super::copy::Copier;
use crate::app::GameWorld;

/// The verbs forwarded: every function of `Damage`.
const VERBS: [&str; 3] = ["Apply", "ApplyPoint", "ApplyRadial"];

/// Replace the scenario's `Damage.<verb>`s with forwards into the gameplay VM.
pub fn register(lua: &Lua, world: &Rc<RefCell<GameWorld>>) -> LuaResult<()> {
    let table: Table = lua.globals().get("Damage")?;
    for verb in VERBS {
        let world = Rc::clone(world);
        let f = lua.create_function(move |lua, args: MultiValue| {
            let world = world.borrow();
            let ran: Option<MultiValue> = world.script_manager().with_vm(|glua| {
                let damage: Table = glua.globals().get("Damage")?;
                let f: Function = damage.get(verb)?;
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
                mlua::Error::RuntimeError(format!("Damage.{verb}: the game's runtime is down"))
            })
        })?;
        table.set(verb, f)?;
    }
    Ok(())
}
