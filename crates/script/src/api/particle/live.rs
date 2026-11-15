//! crates/script/src/api/particle/live.rs — the live-particle pair (#2171):
//! `GetParticles` / `SetParticles`, Unity's `ParticleSystem.GetParticles` /
//! `SetParticles`, so a per-particle rule is plain Lua in a script.
//!
//! A particle table is in world space: `position` and `velocity` (`{ x, y, z }`),
//! `age`, `lifetime`, `size` (start size), `rotation` (degrees), `color` (the
//! `{ r, g, b, a }` start tint) and `custom` (four numbers the engine never reads).
//! `SetParticles` matches the list to the live particles by index; a field an entry
//! leaves out, or gives a NaN or infinite number, keeps its value (an added
//! particle's is `ParticleEmitterComponent::placed`'s). An active `Local` emitter is
//! carried to where it stands before either call, so what a script reads and writes
//! is where the particles draw now, whatever moved the emitter since its tick.

use mlua::{IntoLua, Lua, Table};

use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::api::{put, Reg};
use rusty_components::particle::Particle;
use rusty_scene::Scene;

const XYZ: [&str; 3] = ["x", "y", "z"];
const RGBA: [&str; 4] = ["r", "g", "b", "a"];

/// Register the live-particle pair onto the `Particles` table.
pub(super) fn register(lua: &Lua, table: &Table, scene: ApiCell<Scene>) -> Reg {
    // `GetParticles(id)`: one table per live particle, oldest first; `{}` without
    // an emitter.
    put(
        table,
        "GetParticles",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let mut scene = scene.borrow_mut();
            let id = id.read();
            let (frame, active) = (
                scene.world.emitter_frame(id),
                scene.world.is_active_in_hierarchy(id),
            );
            let list = lua.create_table()?;
            let Some(mut emitter) = scene.world.particles_mut(id) else {
                return Ok(list);
            };
            if active {
                emitter.carry_to(frame);
            }
            for (i, p) in emitter.runtime.particles.iter().enumerate() {
                list.raw_set(i + 1, read(lua, p)?)?;
            }
            Ok(list)
        }),
    )?;

    // `SetParticles(id, list)`: the live set becomes `list` (dead entries and
    // those past `max_particles` dropped, no death sub-emitter fired). Returns the
    // number live; 0 without an emitter. A bad entry is an error that changes
    // nothing.
    put(
        table,
        "SetParticles",
        lua.create_function(move |lua, (id, list): (EntityArg, Table)| {
            let id = id.live("Particles", "SetParticles")?;
            let scene = &*scene.get(lua)?;
            let mut scene = scene.borrow_mut();
            let (frame, active) = (
                scene.world.emitter_frame(id),
                scene.world.is_active_in_hierarchy(id),
            );
            let Some(mut emitter) = scene.world.particles_mut(id) else {
                return Ok(0);
            };
            if active {
                emitter.carry_to(frame);
            }
            let cap = emitter.max_particles as usize;
            let mut live = Vec::with_capacity(list.raw_len().min(cap));
            for i in 0..list.raw_len() {
                if live.len() == cap {
                    break;
                }
                let entry: Table = list.raw_get(i + 1).map_err(|_| {
                    mlua::Error::runtime(format!(
                        "Particles.SetParticles: entry {} is not a particle table",
                        i + 1
                    ))
                })?;
                let old = emitter.runtime.particles.get(i).copied();
                let mut p = old.unwrap_or_else(|| emitter.placed(&frame));
                write(&mut p, &entry)?;
                if p.age < p.lifetime {
                    live.push(p);
                }
            }
            emitter.runtime.particles = live;
            Ok(emitter.runtime.particles.len() as u32)
        }),
    )
}

/// `p` as the table `GetParticles` returns.
fn read(lua: &Lua, p: &Particle) -> mlua::Result<Table> {
    let t = lua.create_table_with_capacity(0, 8)?;
    t.raw_set("position", named(lua, &XYZ, &p.position.to_array())?)?;
    t.raw_set("velocity", named(lua, &XYZ, &p.velocity.to_array())?)?;
    t.raw_set("age", p.age)?;
    t.raw_set("lifetime", p.lifetime)?;
    t.raw_set("size", p.size)?;
    t.raw_set("rotation", p.rotation.to_degrees())?;
    t.raw_set("color", named(lua, &RGBA, &p.color)?)?;
    t.raw_set("custom", lua.create_sequence_from(p.custom)?)?;
    Ok(t)
}

/// The table `{ keys[i] = xs[i] }`.
fn named(lua: &Lua, keys: &[&str], xs: &[f32]) -> mlua::Result<Table> {
    let t = lua.create_table_with_capacity(0, keys.len())?;
    for (k, x) in keys.iter().zip(xs) {
        t.raw_set(*k, *x)?;
    }
    Ok(t)
}

/// Write the fields `t` gives into `p`; what it leaves out, or gives a NaN or an
/// infinity, keeps its value. `rotation` is written only when it differs from
/// `p`'s in degrees, so a round trip keeps the radians bit for bit.
fn write(p: &mut Particle, t: &Table) -> mlua::Result<()> {
    let mut position = p.position.to_array();
    components(t, "position", &XYZ, &mut position)?;
    p.position = position.into();
    let mut velocity = p.velocity.to_array();
    components(t, "velocity", &XYZ, &mut velocity)?;
    p.velocity = velocity.into();
    number(t, "age", &mut p.age)?;
    number(t, "lifetime", &mut p.lifetime)?;
    number(t, "size", &mut p.size)?;
    let mut degrees = p.rotation.to_degrees();
    if number(t, "rotation", &mut degrees)? {
        p.rotation = degrees.to_radians();
    }
    components(t, "color", &RGBA, &mut p.color)?;
    if let Some(custom) = t.get::<Option<Table>>("custom")? {
        for (i, slot) in p.custom.iter_mut().enumerate() {
            number(&custom, i + 1, slot)?;
        }
    }
    Ok(())
}

/// The table at `key` of `t`, if any, written into `slots` by `keys`.
fn components(t: &Table, key: &str, keys: &[&str], slots: &mut [f32]) -> mlua::Result<()> {
    if let Some(v) = t.get::<Option<Table>>(key)? {
        for (k, slot) in keys.iter().zip(slots) {
            number(&v, *k, slot)?;
        }
    }
    Ok(())
}

/// Write the finite number at `key` of `t` into `slot`; whether it changed it.
fn number(t: &Table, key: impl IntoLua, slot: &mut f32) -> mlua::Result<bool> {
    let x = t.get::<Option<f64>>(key)?.map(|x| x as f32);
    match x {
        Some(x) if x.is_finite() && x != *slot => {
            *slot = x;
            Ok(true)
        }
        _ => Ok(false),
    }
}
