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

use glam::Vec3;
use mlua::Lua;

use super::{put, Reg};
use crate::api::ApiCell;
use crate::navigation::NavigationGraph;
use crate::scene::Scene;

/// Register the `Navigation` and `NavMeshAgent` namespaces onto `lua`.
pub fn register(lua: &Lua, scene: ApiCell<Scene>, nav: ApiCell<NavigationGraph>) -> Reg {
    register_navigation(lua, scene, nav)?;
    agent::register(lua, scene, nav)
}

/// `Navigation.GetNextPathStep` over the shared nav graph, plus the per-scene navmesh
/// bake-settings getters/setters (#276).
fn register_navigation(lua: &Lua, scene: ApiCell<Scene>, nav: ApiCell<NavigationGraph>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    put(
        &table,
        "GetNextPathStep",
        lua.create_function(
            move |lua, (cx, cy, cz, tx, ty, tz): (f32, f32, f32, f32, f32, f32)| {
                let nav = &*nav.get(lua)?;
                let step = nav
                    .borrow()
                    .get_next_path_step(Vec3::new(cx, cy, cz), Vec3::new(tx, ty, tz));
                Ok((step.x, step.y, step.z))
            },
        ),
    )?;

    // `Navigation.Bake()` (#1887): a full bake now. A dev build replaces it with the
    // one that also saves `<scene>.navmesh` (`dev::lua_surface::bake`).
    put(
        &table,
        "Bake",
        lua.create_function(move |lua, ()| {
            let (scene, nav) = (&*scene.get(lua)?, &*nav.get(lua)?);
            nav.borrow_mut().bake(&scene.borrow());
            Ok(false)
        }),
    )?;
    settings::register(lua, &table, scene, nav)?;
    query::register(lua, &table, nav)?;
    offmesh::register_navigation(lua, &table, scene, nav)?;
    areas::register_navigation(lua, &table, scene, nav)?;

    lua.globals()
        .set("Navigation", table)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod areas_tests;
#[cfg(test)]
mod query_tests;
