//! src/api/particle/space.rs — the `Particles` simulation-space pair (#1529):
//! `SetSimulationSpace` / `GetSimulationSpace`. The setter is a field write routed
//! through the shared `scene::authoring::particles` op the inspector calls.

use super::with_emitter;
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::api::{put, Reg};
use crate::components::SimulationSpace;
use crate::scene::authoring::particles as particle_ops;
use crate::scene::Scene;

/// Register the simulation-space pair onto the `Particles` table.
pub(super) fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    // `SetSimulationSpace(id, "World"|"Local")`; an unknown name is an error.
    put(
        table,
        "SetSimulationSpace",
        lua.create_function(move |lua, (id, name): (EntityArg, String)| {
            let id = id.live("Particles", "SetSimulationSpace")?;
            let space = SimulationSpace::parse(&name).ok_or_else(|| {
                mlua::Error::RuntimeError(format!(
                    "SetSimulationSpace: unknown simulation space {name:?} (want one of {})",
                    SimulationSpace::names()
                ))
            })?;
            with_emitter(&*scene.get(lua)?, id, |p| {
                particle_ops::set_simulation_space(p, space)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "GetSimulationSpace",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let space = scene
                .world
                .particles(id.read())
                .map(|p| p.simulation_space.name());
            Ok(space)
        }),
    )
}
