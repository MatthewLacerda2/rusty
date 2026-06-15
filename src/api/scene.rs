//! src/api/scene.rs — `Scene` namespace.
//!
//! `FindEntityByName` and `DestroyEntity` (a deactivation, mirroring Unity's
//! deferred destroy). Unity: `SceneManager` / `Object.Destroy`. Scoped callbacks
//! over the borrowed scene + console (#57).

use std::cell::RefCell;

use mlua::{Lua, Scope};

use super::{put, Reg};
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;

/// Register the `Scene` namespace onto `lua`.
pub fn register<'a, 'scope>(
    scope: &Scope<'a, 'scope>,
    lua: &'a Lua,
    scene: &'scope RefCell<&'scope mut Scene>,
    console: &'scope RefCell<&'scope mut ConsoleLogs>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "FindEntityByName",
        scope.create_function(move |_, name: String| {
            Ok(scene.borrow().find_entity_by_name(&name))
        }),
    )?;

    put(
        &table,
        "DestroyEntity",
        scope.create_function(move |_, id: u32| {
            let mut scene = scene.borrow_mut();
            if let Some(mut e) = scene.get_entity_mut(id) {
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
