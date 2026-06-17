//! src/api/particle.rs — `Particles` namespace.
//!
//! Script/REPL/bot control over an entity's `ParticleEmitterComponent`: fire
//! one-off emissions (`Emit`/`Burst`), gate the emitter (`SetActive`), retune the
//! continuous rate (`SetRate`), and read/clear the live state. Emission goes
//! through the component's own seeded spawn path (`emit_at`), so a scripted burst
//! stays bit-for-bit reproducible alongside the sim's own emission.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;

use super::{put, Reg};
use crate::scene::Scene;

/// Register the `Particles` namespace onto `lua`.
#[allow(clippy::too_many_lines)] // grandfathered: burn down in #124
pub fn register(lua: &Lua, scene: &Rc<RefCell<Scene>>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    // Emit `count` particles at the entity's position, right now. Returns the
    // number actually spawned (the max-particle cap may swallow some).
    let s = Rc::clone(scene);
    put(
        &table,
        "Emit",
        lua.create_function(move |_, (id, count): (u32, u32)| {
            let mut scene = s.borrow_mut();
            let spawned = scene.get_entity_mut(id).and_then(|mut e| {
                let origin = e.transform.position;
                e.particles.as_mut().map(|p| p.emit_at(origin, count))
            });
            Ok(spawned.unwrap_or(0))
        }),
    )?;

    // Fire one full configured burst (`burst_count`) immediately, independent of
    // the emit mode or the auto-burst bookkeeping. Returns the number spawned.
    let s = Rc::clone(scene);
    put(
        &table,
        "Burst",
        lua.create_function(move |_, id: u32| {
            let mut scene = s.borrow_mut();
            let spawned = scene.get_entity_mut(id).and_then(|mut e| {
                let origin = e.transform.position;
                e.particles
                    .as_mut()
                    .map(|p| p.emit_at(origin, p.burst_count))
            });
            Ok(spawned.unwrap_or(0))
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "SetActive",
        lua.create_function(move |_, (id, active): (u32, bool)| {
            with_emitter(&s, id, |p| p.active = active);
            Ok(())
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "SetRate",
        lua.create_function(move |_, (id, rate): (u32, f32)| {
            with_emitter(&s, id, |p| p.rate = rate.max(0.0));
            Ok(())
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "IsActive",
        lua.create_function(move |_, id: u32| {
            let scene = s.borrow();
            let active = scene
                .get_entity(id)
                .and_then(|e| e.particles.as_ref().map(|p| p.active));
            Ok(active.unwrap_or(false))
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "GetCount",
        lua.create_function(move |_, id: u32| {
            let scene = s.borrow();
            let count = scene
                .get_entity(id)
                .and_then(|e| e.particles.as_ref().map(|p| p.live_count() as u32));
            Ok(count.unwrap_or(0))
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        &table,
        "Clear",
        lua.create_function(move |_, id: u32| {
            with_emitter(&s, id, |p| p.runtime.particles.clear());
            Ok(())
        }),
    )?;

    lua.globals()
        .set("Particles", table)
        .map_err(|e| e.to_string())
}

/// Mutate the emitter on entity `id` if it has one (no-op otherwise).
fn with_emitter(
    scene: &Rc<RefCell<Scene>>,
    id: u32,
    f: impl FnOnce(&mut crate::scene::ParticleEmitterComponent),
) {
    let mut scene = scene.borrow_mut();
    let Some(mut e) = scene.get_entity_mut(id) else {
        return;
    };
    if let Some(p) = &mut e.particles {
        f(p);
    }
}
