//! src/api/particle.rs — `Particles` namespace.
//!
//! Script/REPL/bot control over an entity's `ParticleEmitterComponent`: fire
//! one-off emissions (`Emit`/`Burst`), gate the emitter (`SetActive`), retune the
//! continuous rate (`SetRate`), reshape it per shot (`SetShape`, `SetDirection`,
//! the start ranges, `SetColor`, `SetSubEmitter` — #439), and read/clear the live
//! state. Emission goes
//! through the component's own seeded spawn path (`emit_at`), so a scripted burst
//! stays bit-for-bit reproducible alongside the sim's own emission.

use std::cell::RefCell;

use mlua::Lua;

use super::{put, Reg};
use crate::components::{EmitFrom, EmitShape, SubEmitTrigger};
use crate::core::curve::{ColorRange, Range};
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
    register_modules(scope, &table, scene)?;
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

/// The #439 module setters: shape, direction, start ranges, colour, sub-emitters.
fn register_modules<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    // `SetShape(id, kind, opts?)` — opts: radius, angle (cone, degrees), x/y/z (box
    // size), surface. Returns false (and changes nothing) for an unknown kind.
    put(
        table,
        "SetShape",
        scope.create_function(|_, (id, kind, opts): (u32, String, Option<mlua::Table>)| {
            let get = |k: &str, d: f32| opts.as_ref().and_then(|o| o.get(k).ok()).unwrap_or(d);
            let size = glam::Vec3::new(get("x", 1.0), get("y", 1.0), get("z", 1.0));
            let surface: bool = opts
                .as_ref()
                .and_then(|o| o.get("surface").ok())
                .unwrap_or(false);
            let from = if surface {
                EmitFrom::Surface
            } else {
                EmitFrom::Volume
            };
            let Some(shape) =
                EmitShape::from_parts(&kind, get("radius", 1.0), get("angle", 25.0), size)
            else {
                return Ok(false);
            };
            with_emitter(scene, id, |p| {
                particle_ops::set_shape(p, shape);
                particle_ops::set_emit_from(p, from);
            });
            Ok(true)
        }),
    )?;
    put(
        table,
        "SetDirection",
        scope.create_function(|_, (id, x, y, z): (u32, f32, f32, f32)| {
            let dir = glam::Vec3::new(x, y, z);
            with_emitter(scene, id, |p| particle_ops::set_direction(p, dir));
            Ok(())
        }),
    )?;
    // `SetLifetime` / `SetSpeed` / `SetSize(id, min, max?)` — `max` omitted is a
    // constant.
    type RangeOp = fn(&mut crate::scene::ParticleEmitterComponent, Range);
    let ranges: [(&str, RangeOp); 3] = [
        ("SetLifetime", particle_ops::set_lifetime),
        ("SetSpeed", particle_ops::set_speed),
        ("SetSize", particle_ops::set_size),
    ];
    for (name, op) in ranges {
        put(
            table,
            name,
            scope.create_function(move |_, (id, min, max): (u32, f32, Option<f32>)| {
                let range = Range::new(min, max.unwrap_or(min));
                with_emitter(scene, id, |p| op(p, range));
                Ok(())
            }),
        )?;
    }
    put(
        table,
        "SetColor",
        scope.create_function(|_, (id, r, g, b, a): (u32, f32, f32, f32, Option<f32>)| {
            let color = ColorRange::constant([r, g, b, a.unwrap_or(1.0)]);
            with_emitter(scene, id, |p| particle_ops::set_color(p, color));
            Ok(())
        }),
    )?;
    // `SetSubEmitter(id, "birth"|"death"|"collision", target?)` — nil clears.
    put(
        table,
        "SetSubEmitter",
        scope.create_function(|_, (id, trigger, target): (u32, String, Option<u32>)| {
            let Some(trigger) = SubEmitTrigger::parse(&trigger) else {
                return Ok(false);
            };
            with_emitter(scene, id, |p| {
                particle_ops::set_sub_emitter(p, trigger, target)
            });
            Ok(true)
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
