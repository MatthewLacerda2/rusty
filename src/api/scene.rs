//! src/api/scene.rs — `Scene` namespace.
//!
//! `FindEntityByName` and `DestroyEntity` (a deactivation, mirroring Unity's
//! deferred destroy). Unity: `SceneManager` / `Object.Destroy`.

use std::cell::RefCell;

use mlua::Lua;

use super::{put, Reg};
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// Register the `Scene` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
    console: &'scope RefCell<ConsoleLogs>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "FindEntityByName",
        scope.create_function(|_, name: String| Ok(scene.borrow().find_entity_by_name(&name))),
    )?;

    put(
        &table,
        "DestroyEntity",
        scope.create_function(|_, id: u32| {
            let mut s = scene.borrow_mut();
            if let Some(mut e) = s.get_entity_mut(id) {
                e.active = false;
                console
                    .borrow_mut()
                    .info(format!("Entity {} destroyed (deactivated)", e.name));
            }
            Ok(())
        }),
    )?;

    lua.globals().set("Scene", table).map_err(|e| e.to_string())
}
