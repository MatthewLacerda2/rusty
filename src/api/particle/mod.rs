//! src/api/particle/mod.rs — `Particles` namespace.
//!
//! Script/REPL/bot control over an entity's `ParticleEmitterComponent`: fire
//! one-off emissions (`Emit`/`Burst`), gate the emitter (`SetActive`), retune the
//! continuous rate (`SetRate`), reshape it per shot (`SetShape`, `SetDirection`,
//! the start ranges, `SetColor`, `SetSubEmitter` — #439), choose how it is drawn
//! (`SetRenderMode`, `SetFlipbook`, `SetSoft`, `SetLit`, … — #440, in `render`), pick
//! the simulation space (`SetSimulationSpace` — #1529, in `space`), and
//! read/clear the live state. Emission goes
//! through the component's own seeded spawn path (`emit_at`), so a scripted burst
//! stays bit-for-bit reproducible alongside the sim's own emission.

mod render;
mod space;

use std::cell::RefCell;

use mlua::Lua;

use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::api::{put, Reg};
use crate::components::{EmitFrom, EmitShape, SubEmitTrigger};
use crate::core::curve::{ColorRange, Range};
use crate::scene::authoring::particles as particle_ops;
use crate::scene::Scene;

/// Register the `Particles` namespace onto `lua`.
pub fn register(lua: &Lua, scene: ApiCell<Scene>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_emission(lua, &table, scene)?;
    register_tuning(lua, &table, scene)?;
    register_shape(lua, &table, scene)?;
    register_start(lua, &table, scene)?;
    register_state(lua, &table, scene)?;
    render::register(lua, &table, scene)?;
    space::register(lua, &table, scene)?;

    lua.globals()
        .set("Particles", table)
        .map_err(|e| e.to_string())
}

/// One-off emissions: `Emit` (count) and `Burst` (the configured burst count).
fn register_emission(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    // Emit `count` particles at the entity's world position, right now. Returns the
    // number actually spawned (the max-particle cap may swallow some).
    put(
        table,
        "Emit",
        lua.create_function(move |lua, (id, count): (EntityArg, u32)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Particles", "Emit")?;
            let mut scene = scene.borrow_mut();
            let origin = scene.world.emitter_frame(id);
            let spawned = scene
                .world
                .particles_mut(id)
                .map(|mut p| p.emit_at(origin, count));
            Ok(spawned.unwrap_or(0))
        }),
    )?;

    // Fire one full configured burst (`burst_count`) immediately, independent of
    // the emit mode or the auto-burst bookkeeping. Returns the number spawned.
    put(
        table,
        "Burst",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Particles", "Burst")?;
            let mut scene = scene.borrow_mut();
            let origin = scene.world.emitter_frame(id);
            let spawned = scene.world.particles_mut(id).map(|mut p| {
                let burst = p.burst_count;
                p.emit_at(origin, burst)
            });
            Ok(spawned.unwrap_or(0))
        }),
    )
}

/// Emitter tuning: `SetActive` (gate) and `SetRate` (continuous rate).
fn register_tuning(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetActive",
        lua.create_function(move |lua, (id, active): (EntityArg, bool)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Particles", "SetActive")?;
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
        lua.create_function(move |lua, (id, rate): (EntityArg, f32)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Particles", "SetRate")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.particles_mut(id) {
                particle_ops::set_rate(&mut c, rate);
            }
            Ok(())
        }),
    )
}

/// The #439 shape setters: `SetShape` and `SetDirection` (the shape's axis).
fn register_shape(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    // `SetShape(id, kind, opts?)` — opts: radius, angle (cone, degrees), x/y/z (box
    // size), surface. Returns false (and changes nothing) for an unknown kind.
    put(
        table,
        "SetShape",
        lua.create_function(
            move |lua, (id, kind, opts): (EntityArg, String, Option<mlua::Table>)| {
                let id = id.live("Particles", "SetShape")?;
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
                with_emitter(&*scene.get(lua)?, id, |p| {
                    particle_ops::set_shape(p, shape);
                    particle_ops::set_emit_from(p, from);
                });
                Ok(true)
            },
        ),
    )?;
    put(
        table,
        "SetDirection",
        lua.create_function(move |lua, (id, x, y, z): (EntityArg, f32, f32, f32)| {
            let id = id.live("Particles", "SetDirection")?;
            let dir = glam::Vec3::new(x, y, z);
            with_emitter(&*scene.get(lua)?, id, |p| {
                particle_ops::set_direction(p, dir)
            });
            Ok(())
        }),
    )
}

/// The #439 start-value setters: ranges, colour, and sub-emitters.
fn register_start(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
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
            lua.create_function(move |lua, (id, min, max): (EntityArg, f32, Option<f32>)| {
                let id = id.live("Particles", name)?;
                let range = Range::new(min, max.unwrap_or(min));
                with_emitter(&*scene.get(lua)?, id, |p| op(p, range));
                Ok(())
            }),
        )?;
    }
    put(
        table,
        "SetColor",
        lua.create_function(
            move |lua, (id, r, g, b, a): (EntityArg, f32, f32, f32, Option<f32>)| {
                let id = id.live("Particles", "SetColor")?;
                let color = ColorRange::constant([r, g, b, a.unwrap_or(1.0)]);
                with_emitter(&*scene.get(lua)?, id, |p| particle_ops::set_color(p, color));
                Ok(())
            },
        ),
    )?;
    // `SetSubEmitter(id, "birth"|"death"|"collision", target?)` — nil clears.
    put(
        table,
        "SetSubEmitter",
        lua.create_function(
            move |lua, (id, trigger, target): (EntityArg, String, EntityArg)| {
                let id = id.live("Particles", "SetSubEmitter")?;
                let target = target.get();
                let Some(trigger) = SubEmitTrigger::parse(&trigger) else {
                    return Ok(false);
                };
                with_emitter(&*scene.get(lua)?, id, |p| {
                    particle_ops::set_sub_emitter(p, trigger, target)
                });
                Ok(true)
            },
        ),
    )
}

/// Live state: `IsActive`, `GetCount`, and `Clear`.
fn register_state(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "IsActive",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let active = scene.world.particles(id.read()).map(|p| p.active);
            Ok(active.unwrap_or(false))
        }),
    )?;

    put(
        table,
        "GetCount",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let count = scene
                .world
                .particles(id.read())
                .map(|p| p.live_count() as u32);
            Ok(count.unwrap_or(0))
        }),
    )?;

    put(
        table,
        "Clear",
        lua.create_function(move |lua, id: EntityArg| {
            let id = id.live("Particles", "Clear")?;
            with_emitter(&*scene.get(lua)?, id, |p| p.runtime.particles.clear());
            Ok(())
        }),
    )
}

/// Mutate the emitter on entity `id` if it has one (no-op otherwise).
pub(super) fn with_emitter(
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
