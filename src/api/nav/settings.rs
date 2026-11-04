//! `Navigation`'s per-scene navmesh bake settings (#276) and bounds (#452): getters
//! over `scene.nav_settings`, and setters that write it and re-bake the shared graph.

use std::cell::RefCell;

use super::super::{put, Reg};
use crate::api::ApiCell;
use crate::navigation::{NavBounds, NavMeshSettings, NavigationGraph};
use crate::scene::Scene;

/// Register the settings and bounds functions onto the `Navigation` table.
pub(super) fn register(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    nav: ApiCell<NavigationGraph>,
) -> Reg {
    register_settings_getters(lua, table, scene)?;
    register_settings_setters(lua, table, scene, nav)?;
    register_bounds(lua, table, scene, nav)
}

/// The read-only getters over `scene.nav_settings` (#276, + `agent_height` #278).
fn register_settings_getters(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetAgentRadius",
        lua.create_function(move |lua, ()| Ok(scene.get(lua)?.borrow().nav_settings.agent_radius)),
    )?;
    put(
        table,
        "GetAgentHeight",
        lua.create_function(move |lua, ()| Ok(scene.get(lua)?.borrow().nav_settings.agent_height)),
    )?;
    put(
        table,
        "GetMaxSlope",
        lua.create_function(move |lua, ()| Ok(scene.get(lua)?.borrow().nav_settings.max_slope)),
    )?;
    put(
        table,
        "GetMaxStep",
        lua.create_function(move |lua, ()| Ok(scene.get(lua)?.borrow().nav_settings.max_step)),
    )?;
    put(
        table,
        "GetGridSpacing",
        lua.create_function(move |lua, ()| Ok(scene.get(lua)?.borrow().nav_settings.grid_spacing)),
    )
}

/// The setters (#276): write `scene.nav_settings`, then re-bake the shared graph from the
/// scene so the knob is consumed (the bake is the read-site) — the same effect the scene
/// inspector's Navmesh section has (editor↔API parity). `agent_radius` is live (#277): the
/// re-bake erodes the walkable surface inward by the radius, so a larger radius closes thin
/// passages and pulls the surface off walls. `agent_height` is live too (#278): the re-bake
/// drops spans whose open space is below the height, so a taller agent loses access
/// to low overhangs / crawlspaces.
fn register_settings_setters(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    nav: ApiCell<NavigationGraph>,
) -> Reg {
    put(
        table,
        "SetAgentRadius",
        lua.create_function(move |lua, r: f32| {
            set_and_rebake(&*scene.get(lua)?, &*nav.get(lua)?, |s| s.agent_radius = r)
        }),
    )?;
    put(
        table,
        "SetAgentHeight",
        lua.create_function(move |lua, h: f32| {
            set_and_rebake(&*scene.get(lua)?, &*nav.get(lua)?, |s| s.agent_height = h)
        }),
    )?;
    put(
        table,
        "SetMaxSlope",
        lua.create_function(move |lua, v: f32| {
            set_and_rebake(&*scene.get(lua)?, &*nav.get(lua)?, |s| s.max_slope = v)
        }),
    )?;
    put(
        table,
        "SetMaxStep",
        lua.create_function(move |lua, v: f32| {
            set_and_rebake(&*scene.get(lua)?, &*nav.get(lua)?, |s| s.max_step = v)
        }),
    )?;
    put(
        table,
        "SetGridSpacing",
        lua.create_function(move |lua, v: f32| {
            set_and_rebake(&*scene.get(lua)?, &*nav.get(lua)?, |s| s.grid_spacing = v)
        }),
    )
}

/// The navmesh bounds (#452): the XZ area the grid covers. `GetBounds` reads the live
/// graph — the bounds the last bake actually used, snapped to the grid spacing — plus
/// whether they were authored. `SetBounds` authors an override (Unity's nav volume) and
/// `ClearBounds` returns to deriving them from the static geometry; both re-bake, as the
/// scene inspector's Navmesh section does (editor↔API parity).
fn register_bounds(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    nav: ApiCell<NavigationGraph>,
) -> Reg {
    put(
        table,
        "GetBounds",
        lua.create_function(move |lua, ()| {
            let b = nav.get(lua)?.borrow().bounds();
            let authored = scene.get(lua)?.borrow().nav_settings.bounds.is_some();
            Ok((b.min_x, b.max_x, b.min_z, b.max_z, authored))
        }),
    )?;
    put(
        table,
        "SetBounds",
        lua.create_function(
            move |lua, (min_x, max_x, min_z, max_z): (f32, f32, f32, f32)| {
                let b = NavBounds {
                    min_x,
                    max_x,
                    min_z,
                    max_z,
                };
                if !b.is_valid() {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Navigation.SetBounds: need finite bounds with min < max, got \
                         x [{min_x}, {max_x}] z [{min_z}, {max_z}]"
                    )));
                }
                set_and_rebake(&*scene.get(lua)?, &*nav.get(lua)?, |s| s.bounds = Some(b))
            },
        ),
    )?;
    put(
        table,
        "ClearBounds",
        lua.create_function(move |lua, ()| {
            set_and_rebake(&*scene.get(lua)?, &*nav.get(lua)?, |s| s.bounds = None)
        }),
    )
}

/// Apply a mutation to `scene.nav_settings`, then re-bake the shared nav graph from the
/// scene so the write is observed. The scene's mutable borrow is scoped to the write
/// alone, so the immutable borrow the bake needs never overlaps it.
pub(super) fn set_and_rebake(
    scene: &RefCell<Scene>,
    nav: &RefCell<NavigationGraph>,
    apply: impl FnOnce(&mut NavMeshSettings),
) -> mlua::Result<()> {
    apply(&mut scene.borrow_mut().nav_settings);
    nav.borrow_mut().bake(&scene.borrow());
    Ok(())
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod settings_tests;
