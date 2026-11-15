//! crates/script/src/api/particle/live.rs — the live-particle calls (#2171):
//! `GetParticles` / `SetParticles`, Unity's `ParticleSystem.GetParticles` /
//! `SetParticles`, and `ForEach`, one reused table per particle for the hot rule.
//!
//! A particle table is in world space: `position` and `velocity` (`{ x, y, z }`),
//! `age`, `lifetime`, `size` (start size), `rotation` (degrees), `color` (the
//! `{ r, g, b, a }` start tint) and `custom` (four numbers the engine never reads).
//! The tables are built and read by `live.lua`; this half moves the particles
//! across as one packed string each way ([`read`], [`write`]), since a Rust call
//! per field costs several times the whole Lua side. A field written back as a
//! NaN or an infinity (what `live.lua` sends for one left out) keeps its value; an
//! added particle's is `ParticleEmitterComponent::placed`'s. An active `Local`
//! emitter is carried to where it stands before every read and write, so what a
//! script reads and writes is where the particles draw now.

use mlua::{Function, Lua, Table};

use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::api::Reg;
use rusty_components::particle::{EmitterFrame, Particle, ParticleEmitterComponent};
use rusty_scene::Scene;

/// The f32s of one particle on the wire (`live.lua`'s layout).
const FIELDS: usize = 18;
/// The bytes of one particle on the wire, and of one record written back (a u32
/// source index first).
const PARTICLE: usize = FIELDS * 4;
const RECORD: usize = 4 + PARTICLE;

/// Register `GetParticles`, `SetParticles` and `ForEach` onto the `Particles` table.
pub(super) fn register(lua: &Lua, table: &Table, scene: ApiCell<Scene>) -> Reg {
    let read = lua
        .create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let bytes = with_carried(&mut scene.borrow_mut(), id.read(), |e, _| {
                packed(&e.runtime.particles)
            });
            let count = bytes.as_ref().map_or(0, |b| b.len() / PARTICLE);
            Ok((lua.create_string(bytes.unwrap_or_default())?, count))
        })
        .map_err(|e| e.to_string())?;
    let write = lua
        .create_function(
            move |lua, (id, verb, records, count): (EntityArg, String, mlua::LuaString, usize)| {
                let id = id.live("Particles", &verb)?;
                let scene = &*scene.get(lua)?;
                let records = records.as_bytes();
                let live = with_carried(&mut scene.borrow_mut(), id, |e, frame| {
                    write_records(e, frame, &records, count)
                });
                Ok(live.unwrap_or(0))
            },
        )
        .map_err(|e| e.to_string())?;
    let (get, set, for_each): (Function, Function, Function) = lua
        .load(include_str!("live.lua"))
        .set_name("=particles_live")
        .call((read, write))
        .map_err(|e| e.to_string())?;
    for (name, f) in [
        ("GetParticles", get),
        ("SetParticles", set),
        ("ForEach", for_each),
    ] {
        table.set(name, f).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Run `f` on `id`'s emitter after carrying an active `Local` one to where it
/// stands, with its frame; `None` without an emitter.
fn with_carried<R>(
    scene: &mut Scene,
    id: u32,
    f: impl FnOnce(&mut ParticleEmitterComponent, &EmitterFrame) -> R,
) -> Option<R> {
    let frame = scene.world.emitter_frame(id);
    let active = scene.world.is_active_in_hierarchy(id);
    let mut emitter = scene.world.particles_mut(id)?;
    if active {
        emitter.carry_to(frame);
    }
    Some(f(&mut emitter, &frame))
}

/// `particles` on the wire.
fn packed(particles: &[Particle]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(particles.len() * PARTICLE);
    for p in particles {
        for x in fields(p) {
            bytes.extend_from_slice(&x.to_le_bytes());
        }
    }
    bytes
}

/// The 4 bytes at `at` of `b`.
fn word(b: &[u8], at: usize) -> [u8; 4] {
    [b[at], b[at + 1], b[at + 2], b[at + 3]]
}

/// `p`'s fields in wire order, its rotation in degrees.
fn fields(p: &Particle) -> [f32; FIELDS] {
    let ([px, py, pz], [vx, vy, vz]) = (p.position.to_array(), p.velocity.to_array());
    let ([r, g, b, a], [c1, c2, c3, c4]) = (p.color, p.custom);
    let rotation = p.rotation.to_degrees();
    [
        px, py, pz, vx, vy, vz, p.age, p.lifetime, p.size, rotation, r, g, b, a, c1, c2, c3, c4,
    ]
}

/// Replace `e`'s live particles, under its frame `frame`, with `count` records
/// (a u32 source index and [`FIELDS`] f32s each): dead ones and those past `max_particles` dropped, no
/// death sub-emitter fired. Returns the number live.
fn write_records(
    e: &mut ParticleEmitterComponent,
    frame: &EmitterFrame,
    records: &[u8],
    count: usize,
) -> u32 {
    let cap = e.max_particles as usize;
    let count = count.min(records.len() / RECORD);
    let mut live = Vec::with_capacity(count.min(cap));
    for r in 0..count {
        if live.len() == cap {
            break;
        }
        let at = r * RECORD;
        let src = u32::from_le_bytes(word(records, at)) as usize;
        let mut values = [0.0f32; FIELDS];
        for (k, v) in values.iter_mut().enumerate() {
            *v = f32::from_le_bytes(word(records, at + 4 + k * 4));
        }
        let old = e.runtime.particles.get(src).copied();
        let mut p = old.unwrap_or_else(|| e.placed(frame));
        apply(&mut p, &values);
        if p.age < p.lifetime {
            live.push(p);
        }
    }
    e.runtime.particles = live;
    e.runtime.particles.len() as u32
}

/// Write `values` (wire order) into `p`; a NaN or an infinity keeps the field.
/// The rotation is written only when it differs from `p`'s in degrees, so a
/// round trip keeps the radians bit for bit.
fn apply(p: &mut Particle, values: &[f32; FIELDS]) {
    let mut now = fields(p);
    let degrees = now[9];
    for (slot, &v) in now.iter_mut().zip(values) {
        if v.is_finite() {
            *slot = v;
        }
    }
    let [px, py, pz, vx, vy, vz, age, lifetime, size, rotation, r, g, b, a, c1, c2, c3, c4] = now;
    p.position = [px, py, pz].into();
    p.velocity = [vx, vy, vz].into();
    (p.age, p.lifetime, p.size) = (age, lifetime, size);
    if rotation != degrees {
        p.rotation = rotation.to_radians();
    }
    p.color = [r, g, b, a];
    p.custom = [c1, c2, c3, c4];
}
