//! src/api/particle.rs — `Particles` namespace.
//!
//! Script/REPL/bot control over an entity's `ParticleEmitterComponent`: fire
//! one-off emissions (`Emit`/`Burst`), gate the emitter (`SetActive`), retune the
//! continuous rate (`SetRate`), and read/clear the live state. Emission goes
//! through the component's own seeded spawn path (`emit_at`), so a scripted burst
//! stays bit-for-bit reproducible alongside the sim's own emission.

use std::cell::RefCell;

use mlua::Lua;

use super::{put, Reg};
use crate::scene::authoring::particles as particle_ops;
use crate::scene::Scene;

/// Register the `Particles` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_emission(scope, &table, scene)?;
    register_tuning(scope, &table, scene)?;
    register_state(scope, &table, scene)?;

    lua.globals()
        .set("Particles", table)
        .map_err(|e| e.to_string())
}

/// One-off emissions: `Emit` (count) and `Burst` (the configured burst count).
fn register_emission<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    // Emit `count` particles at the entity's position, right now. Returns the
    // number actually spawned (the max-particle cap may swallow some).
    put(
        table,
        "Emit",
        scope.create_function(|_, (id, count): (u32, u32)| {
            let mut scene = scene.borrow_mut();
            let origin = scene.world.transform(id).map(|t| t.position);
            let spawned = origin.and_then(|origin| {
                scene
                    .world
                    .particles_mut(id)
                    .map(|mut p| p.emit_at(origin, count))
            });
            Ok(spawned.unwrap_or(0))
        }),
    )?;

    // Fire one full configured burst (`burst_count`) immediately, independent of
    // the emit mode or the auto-burst bookkeeping. Returns the number spawned.
    put(
        table,
        "Burst",
        scope.create_function(|_, id: u32| {
            let mut scene = scene.borrow_mut();
            let origin = scene.world.transform(id).map(|t| t.position);
            let spawned = origin.and_then(|origin| {
                scene.world.particles_mut(id).map(|mut p| {
                    let burst = p.burst_count;
                    p.emit_at(origin, burst)
                })
            });
            Ok(spawned.unwrap_or(0))
        }),
    )
}

/// Emitter tuning: `SetActive` (gate) and `SetRate` (continuous rate).
fn register_tuning<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetActive",
        scope.create_function(|_, (id, active): (u32, bool)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.particles_mut(id) {
                particle_ops::set_active(&mut c, active);
            }
            Ok(())
        }),
    )?;

    put(
        table,
        "SetRate",
        scope.create_function(|_, (id, rate): (u32, f32)| {
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.particles_mut(id) {
                particle_ops::set_rate(&mut c, rate);
            }
            Ok(())
        }),
    )
}

/// Live state: `IsActive`, `GetCount`, and `Clear`.
fn register_state<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "IsActive",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            let active = scene.world.particles(id).map(|p| p.active);
            Ok(active.unwrap_or(false))
        }),
    )?;

    put(
        table,
        "GetCount",
        scope.create_function(|_, id: u32| {
            let scene = scene.borrow();
            let count = scene.world.particles(id).map(|p| p.live_count() as u32);
            Ok(count.unwrap_or(0))
        }),
    )?;

    put(
        table,
        "Clear",
        scope.create_function(|_, id: u32| {
            with_emitter(scene, id, |p| p.runtime.particles.clear());
            Ok(())
        }),
    )
}

/// Mutate the emitter on entity `id` if it has one (no-op otherwise).
fn with_emitter(
    scene: &RefCell<Scene>,
    id: u32,
    f: impl FnOnce(&mut crate::scene::ParticleEmitterComponent),
) {
    let mut scene = scene.borrow_mut();
    let guard = scene.world.particles_mut(id);
    if let Some(mut p) = guard {
        f(&mut p);
    }
}
