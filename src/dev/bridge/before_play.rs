//! What a scenario may not schedule before its first `Harness.Step` (#1384).
//!
//! The harness asks for Play when it is built, but the world enters it on the first
//! tick, so a scenario can set the scene up (`Harness.AttachPlayerBot` swaps the
//! Player's script before any loads) in edit mode. Entering Play makes the gameplay
//! runtime fresh: it clears the timers and restarts `Random` from its default seed.
//! A timer, tween or seed set before that would be wiped without a word, so those
//! verbs are refused there with an error that says to step first.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Function, Lua, MultiValue, Result as LuaResult, Table};

use crate::app::GameWorld;

/// `(namespace, verbs)` whose effect entering Play discards.
const WIPED_BY_PLAY: &[(&str, &[&str])] = &[
    ("Timer", &["Invoke", "InvokeRepeating", "StartCoroutine"]),
    ("Tween", &["To", "Sequence"]),
    ("Random", &["SetSeed"]),
];

/// Wrap each [`WIPED_BY_PLAY`] verb on `lua`'s globals, after the API registered
/// them, so a call made before the world is playing errors instead.
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
