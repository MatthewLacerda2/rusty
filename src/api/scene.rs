//! src/api/scene.rs — `Scene` namespace.
//!
//! `FindEntityByName` and `DestroyEntity` (a deactivation, mirroring Unity's
//! deferred destroy). Unity: `SceneManager` / `Object.Destroy`.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;

use super::{put, Reg};
use crate::core::scene::Scene;
use crate::scripting::ConsoleLogs;

/// Register the `Scene` namespace onto `lua`.
pub fn register(lua: &Lua, scene: &Rc<RefCell<Scene>>, console: &Rc<RefCell<ConsoleLogs>>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    let s = Rc::clone(scene);
    put(
        &table,
        "FindEntityByName",
        lua.create_function(move |_, name: String| Ok(s.borrow().find_entity_by_name(&name))),
    )?;

    let s = Rc::clone(scene);
    let c = Rc::clone(console);
    put(
        &table,
        "DestroyEntity",
        lua.create_function(move |_, id: u32| {
            let mut scene = s.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
                e.active = false;
                c.borrow_mut()
                    .info(format!("Entity {} destroyed (deactivated)", e.name));
            }
            Ok(())
        }),
    )?;

    lua.globals().set("Scene", table).map_err(|e| e.to_string())
}
