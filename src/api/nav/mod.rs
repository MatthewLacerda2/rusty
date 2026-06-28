//! src/api/nav/ — `Navigation` + `NavMeshAgent` namespaces.
//!
//! `Navigation` queries the shared nav graph (`GetNextPathStep`) and exposes the
//! per-scene navmesh bake settings (#276); `agent` registers `NavMeshAgent.*`, which
//! reads and writes an entity's optional nav-agent component. Split into this `mod.rs`
//! (the `Navigation` namespace + settings) and `agent.rs` (the `NavMeshAgent` surface)
//! to stay under the size cap while keeping each Lua namespace one cohesive unit.

mod agent;

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;

use super::{put, Reg};
use crate::navigation::{NavMeshSettings, NavigationGraph};
use crate::scene::Scene;

/// Register the `Navigation` and `NavMeshAgent` namespaces onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
    nav: &'scope RefCell<NavigationGraph>,
) -> Reg {
    register_navigation(lua, scope, scene, nav)?;
    agent::register(lua, scope, scene)
}

/// `Navigation.GetNextPathStep` over the shared nav graph, plus the per-scene navmesh
/// bake-settings getters/setters (#276).
fn register_navigation<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
    nav: &'scope RefCell<NavigationGraph>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "GetNextPathStep",
        scope.create_function(
            |_, (cx, cy, cz, tx, ty, tz): (f32, f32, f32, f32, f32, f32)| {
                let step = nav
                    .borrow()
                    .get_next_path_step(Vec3::new(cx, cy, cz), Vec3::new(tx, ty, tz));
                Ok((step.x, step.y, step.z))
            },
        ),
    )?;

    register_settings_getters(scope, &table, scene)?;
    register_settings_setters(scope, &table, scene, nav)?;

    lua.globals()
        .set("Navigation", table)
        .map_err(|e| e.to_string())
}

/// The read-only getters over `scene.nav_settings` (#276, + `agent_height` #278).
fn register_settings_getters<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GetAgentRadius",
        scope.create_function(|_, ()| Ok(scene.borrow().nav_settings.agent_radius)),
    )?;
    put(
        table,
        "GetAgentHeight",
        scope.create_function(|_, ()| Ok(scene.borrow().nav_settings.agent_height)),
    )?;
    put(
        table,
        "GetMaxSlope",
        scope.create_function(|_, ()| Ok(scene.borrow().nav_settings.max_slope)),
    )?;
    put(
        table,
        "GetMaxStep",
        scope.create_function(|_, ()| Ok(scene.borrow().nav_settings.max_step)),
    )?;
    put(
        table,
        "GetGridSpacing",
        scope.create_function(|_, ()| Ok(scene.borrow().nav_settings.grid_spacing)),
    )
}

/// The setters (#276): write `scene.nav_settings`, then re-bake the shared graph from the
/// scene so the knob is consumed (the bake is the read-site) — the same effect the scene
/// inspector's Navmesh section has (editor↔API parity). `agent_radius` is live (#277): the
/// re-bake erodes the walkable surface inward by the radius, so a larger radius closes thin
/// passages and pulls the surface off walls. `agent_height` is live too (#278): the re-bake
/// carves cells whose overhead clearance is below the height, so a taller agent loses access
/// to low overhangs / crawlspaces.
fn register_settings_setters<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    nav: &'scope RefCell<NavigationGraph>,
) -> Reg {
    put(
        table,
        "SetAgentRadius",
        scope.create_function(move |_, r: f32| set_and_rebake(scene, nav, |s| s.agent_radius = r)),
    )?;
    put(
        table,
        "SetAgentHeight",
        scope.create_function(move |_, h: f32| set_and_rebake(scene, nav, |s| s.agent_height = h)),
    )?;
    put(
        table,
        "SetMaxSlope",
        scope.create_function(move |_, v: f32| set_and_rebake(scene, nav, |s| s.max_slope = v)),
    )?;
    put(
        table,
        "SetMaxStep",
        scope.create_function(move |_, v: f32| set_and_rebake(scene, nav, |s| s.max_step = v)),
    )?;
    put(
        table,
        "SetGridSpacing",
        scope.create_function(move |_, v: f32| set_and_rebake(scene, nav, |s| s.grid_spacing = v)),
    )
}

/// Apply a mutation to `scene.nav_settings`, then re-bake the shared nav graph from the
/// scene so the write is observed. The scene's mutable borrow is scoped to the write
/// alone, so the immutable borrow the bake needs never overlaps it.
fn set_and_rebake(
    scene: &RefCell<Scene>,
    nav: &RefCell<NavigationGraph>,
    apply: impl FnOnce(&mut NavMeshSettings),
) -> mlua::Result<()> {
    apply(&mut scene.borrow_mut().nav_settings);
    nav.borrow_mut().bake(&scene.borrow());
    Ok(())
}

#[cfg(test)]
mod settings_tests;
