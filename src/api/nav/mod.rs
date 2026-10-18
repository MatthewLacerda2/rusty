//! src/api/nav/ — `Navigation` + `NavMeshAgent` namespaces.
//!
//! `Navigation` queries the shared nav graph (`GetNextPathStep`, and the path queries in
//! `query`, #458) and exposes the
//! per-scene navmesh bake settings (#276); `agent` registers `NavMeshAgent.*`, which
//! reads and writes an entity's optional nav-agent component. Split into this `mod.rs`
//! (the `Navigation` namespace), `settings.rs` (its bake settings and bounds) and
//! `agent.rs` (the `NavMeshAgent` surface), each Lua namespace one cohesive unit.

mod agent;
mod areas;
mod offmesh;
mod path_state;
mod query;
mod settings;
mod turning;

use std::cell::RefCell;

use glam::Vec3;
use mlua::Lua;

use super::{put, Reg};
use crate::navigation::NavigationGraph;
use crate::scene::Scene;

/// Register the `Navigation` and `NavMeshAgent` namespaces onto `lua`.
pub fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    scene: &'scope RefCell<Scene>,
    nav: &'scope RefCell<NavigationGraph>,
) -> Reg {
    register_navigation(lua, scope, scene, nav)?;
    agent::register(lua, scope, scene, nav)
}

/// `Navigation.GetNextPathStep` over the shared nav graph, plus the per-scene navmesh
/// bake-settings getters/setters (#276).
fn register_navigation<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
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

    settings::register(scope, &table, scene, nav)?;
    query::register(scope, &table, nav)?;
    offmesh::register_navigation(scope, &table, scene, nav)?;
    areas::register_navigation(scope, &table, scene, nav)?;

    lua.globals()
        .set("Navigation", table)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod areas_tests;
#[cfg(test)]
mod query_tests;
