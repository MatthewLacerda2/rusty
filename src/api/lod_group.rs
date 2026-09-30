//! src/api/lod_group.rs — `LODGroup` namespace (#472).
//!
//! Get/Set over an entity's `LodGroupComponent`: its size, its levels' screen
//! heights and renderer lists, and adding/removing levels. Every setter routes
//! through the shared `scene::authoring::lod_group` ops the inspector card uses.
//! Levels are indexed from **0** (`LOD0` is the finest), matching the `_LOD0`
//! naming. Getters return a neutral default (`0`, `nil`, an empty table) without an
//! LODGroup or for an out-of-range level; setters are then no-ops. Which level is
//! *shown* is decided by the renderer each frame and is deliberately not readable
//! here: the sim never depends on it.

use std::cell::RefCell;

use mlua::{Lua, Table};

use super::{put, Reg};
use crate::components::LodGroupComponent;
use crate::scene::authoring::lod_group as ops;
use crate::scene::Scene;

type SceneCell<'s> = &'s RefCell<Scene>;

/// Register the `LODGroup` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let t = lua.create_table().map_err(|e| e.to_string())?;
    register_group(scope, &t, scene)?;
    register_levels(scope, &t, scene)?;
    lua.globals().set("LODGroup", t).map_err(|e| e.to_string())
}

/// `id`'s LODGroup, cloned, or `None` without one.
fn get(scene: SceneCell, id: u32) -> Option<LodGroupComponent> {
    scene.borrow().world.lod_group(id).map(|g| g.clone())
}

/// Apply `f` to `id`'s LODGroup, when it has one; `None` without one.
fn set<T>(scene: SceneCell, id: u32, f: impl FnOnce(&mut LodGroupComponent) -> T) -> Option<T> {
    let mut s = scene.borrow_mut();
    let mut g = s.world.lod_group_mut(id)?;
    Some(f(&mut g))
}

/// `Get/SetSize`, `GetLevelCount`, `AddLevel`, `RemoveLevel`.
fn register_group<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    t: &Table<'lua>,
    scene: SceneCell<'scope>,
) -> Reg {
    let f = scope.create_function(move |_, id: u32| Ok(get(scene, id).map_or(0.0, |g| g.size)));
    put(t, "GetSize", f)?;
    let f = scope.create_function(move |_, (id, size): (u32, f32)| {
        set(scene, id, |g| ops::set_size(g, size));
        Ok(())
    });
    put(t, "SetSize", f)?;
    let f =
        scope.create_function(move |_, id: u32| Ok(get(scene, id).map_or(0, |g| g.levels.len())));
    put(t, "GetLevelCount", f)?;
    let f = scope.create_function(move |_, id: u32| Ok(set(scene, id, ops::add_level)));
    put(t, "AddLevel", f)?;
    let f = scope.create_function(move |_, (id, level): (u32, usize)| {
        set(scene, id, |g| ops::remove_level(g, level));
        Ok(())
    });
    put(t, "RemoveLevel", f)
}

/// `Get/SetLevelHeight`, `Get/SetRenderers`.
fn register_levels<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    t: &Table<'lua>,
    scene: SceneCell<'scope>,
) -> Reg {
    let f = scope.create_function(move |_, (id, level): (u32, usize)| {
        let g = get(scene, id);
        Ok(g.and_then(|g| g.levels.get(level).map(|l| l.screen_height)))
    });
    put(t, "GetLevelHeight", f)?;
    let f = scope.create_function(move |_, (id, level, h): (u32, usize, f32)| {
        set(scene, id, |g| ops::set_level_height(g, level, h));
        Ok(())
    });
    put(t, "SetLevelHeight", f)?;
    let f = scope.create_function(move |_, (id, level): (u32, usize)| {
        let g = get(scene, id);
        Ok(
            g.and_then(|g| g.levels.get(level).map(|l| l.renderers.clone()))
                .unwrap_or_default(),
        )
    });
    put(t, "GetRenderers", f)?;
    let f = scope.create_function(move |_, (id, level, ids): (u32, usize, Vec<u32>)| {
        set(scene, id, |g| ops::set_renderers(g, level, ids));
        Ok(())
    });
    put(t, "SetRenderers", f)
}
