use super::with_emitter;
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::api::{put, Reg};
use rusty_components::{ParticleSimulation, SimulationSpace};
use rusty_edit::authoring::particles as particle_ops;
use rusty_scene::Scene;

pub(super) fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    register_target(lua, table, scene)?;
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

fn register_target(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetSimulation",
        lua.create_function(move |lua, (id, name): (EntityArg, String)| {
            let id = id.live("Particles", "SetSimulation")?;
            let target = ParticleSimulation::parse(&name).ok_or_else(|| {
                mlua::Error::RuntimeError(format!(
                    "SetSimulation: unknown simulation {name:?} (want one of {})",
                    ParticleSimulation::names()
                ))
            })?;
            with_emitter(&*scene.get(lua)?, id, |p| {
                particle_ops::set_simulation(p, target)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "GetSimulation",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let target = scene
                .world
                .particles(id.read())
                .map(|p| p.simulation.name());
            Ok(target)
        }),
    )
}
