//! src/api/canvas_group.rs — `CanvasGroup` namespace (#418).
//!
//! Get/Set over an entity's `CanvasGroupComponent`: the subtree `Alpha` (multiplied
//! into every graphic below) and the `Interactable` / `BlocksRaycasts` flags
//! pointer dispatch reads (#420). Setters route through the shared
//! `scene::authoring::canvas_group` ops the inspector card uses. Getters return
//! `1` / `false` without a CanvasGroup; setters are then no-ops.

use std::cell::RefCell;

use mlua::Lua;

use super::{put, Reg};
use crate::components::CanvasGroupComponent;
use crate::scene::authoring::canvas_group as group_ops;
use crate::scene::Scene;

type BoolGet = fn(&CanvasGroupComponent) -> bool;
type BoolSet = fn(&mut CanvasGroupComponent, bool);

/// `(suffix, getter, setter)` for the boolean `Get<X>` / `Set<X>` pairs.
const BOOLS: [(&str, BoolGet, BoolSet); 2] = [
    (
        "Interactable",
        |g| g.interactable,
        group_ops::set_interactable,
    ),
    (
        "BlocksRaycasts",
        |g| g.blocks_raycasts,
        group_ops::set_blocks_raycasts,
    ),
];

/// Register the `CanvasGroup` namespace onto `lua`.
pub fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    put(
        &table,
        "GetAlpha",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            Ok(scene.world.canvas_group(id).map_or(1.0, |g| g.alpha))
        }),
    )?;
    put(
        &table,
        "SetAlpha",
        scope.create_function(|_, (id, alpha): (u32, f32)| {
            if let Some(mut g) = scene.borrow_mut().world.canvas_group_mut(id) {
                group_ops::set_alpha(&mut g, alpha);
            }
            Ok(())
        }),
    )?;
    for (suffix, get, set) in BOOLS {
        register_bool(scope, &table, scene, suffix, get, set)?;
    }
    lua.globals()
        .set("CanvasGroup", table)
        .map_err(|e| e.to_string())
}

/// `Get<suffix>(id) -> bool` / `Set<suffix>(id, bool)`.
fn register_bool<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    suffix: &str,
    get: BoolGet,
    set: BoolSet,
) -> Reg {
    put(
        table,
        &format!("Get{suffix}"),
        scope.create_function(move |_, id: u32| {
            let scene = scene.borrow();
            Ok(scene.world.canvas_group(id).is_some_and(|g| get(&g)))
        }),
    )?;
    put(
        table,
        &format!("Set{suffix}"),
        scope.create_function(move |_, (id, value): (u32, bool)| {
            if let Some(mut g) = scene.borrow_mut().world.canvas_group_mut(id) {
                set(&mut g, value);
            }
            Ok(())
        }),
    )
}
