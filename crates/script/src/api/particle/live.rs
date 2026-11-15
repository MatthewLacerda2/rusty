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
//! added particle's is `ParticleEmitterComponent::placed`'s. A table keeps the
//! identity of the particle it was read from (`live.lua` sends its source with
//! it), so a script that removes or reorders entries keeps each survivor's hidden
//! fields. A `Gpu` emitter's particles are the renderer's (#2170): `GetParticles`
//! reads none, and `SetParticles` warns once and changes nothing. A `Local` emitter is carried to where it stands before every read and
//! write, so what a script reads and writes is where the particles draw now.

use mlua::{Function, Lua, Table};

use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::api::Reg;
use rusty_components::particle::{EmitterFrame, Particle, ParticleEmitterComponent};
use rusty_scene::Scene;

/// The f32s of one particle on the wire, named in wire order. `live.lua` gets
/// the names and the two formats below as upvalues and refuses to load if its
/// own order differs, so this list is the layout's one source.
const NAMES: [&str; FIELDS] = [
    "position.x",
    "position.y",
    "position.z",
    "velocity.x",
    "velocity.y",
    "velocity.z",
    "age",
    "lifetime",
    "size",
    "rotation",
    "color.r",
    "color.g",
    "color.b",
    "color.a",
    "custom.1",
    "custom.2",
    "custom.3",
    "custom.4",
];
const FIELDS: usize = 18;
/// The bytes of one particle on the wire, and of one record written back: a u32
/// source (the live particle a table was read from, [`NO_SOURCE`] for one the
/// script built) and the f32 age that particle had when it was read, first.
const PARTICLE: usize = FIELDS * 4;
const RECORD: usize = 8 + PARTICLE;
/// A record's source when the table did not come from the emitter's newest read.
const NO_SOURCE: u32 = u32::MAX;

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
            move |lua, (id, verb, records): (EntityArg, String, mlua::LuaString)| {
                let id = id.live("Particles", &verb)?;
                let scene = &*scene.get(lua)?;
                let records = records.as_bytes();
                if records.len() % RECORD != 0 {
                    return Err(mlua::Error::RuntimeError(format!(
                        "Particles.{verb}: {} bytes of records is not a whole number of \
                         {RECORD}-byte records (live.lua and live.rs disagree on the layout)",
                        records.len()
                    )));
                }
                let live = with_carried(&mut scene.borrow_mut(), id, |e, frame| match e.on_gpu() {
                    false => write_records(e, frame, &records),
                    true => refuse(e, &verb, id),
                });
                Ok(live.unwrap_or(0))
            },
        )
        .map_err(|e| e.to_string())?;
    let layout = (
        format!("<{}", "f".repeat(FIELDS)),
        format!("<I4f{}", "f".repeat(FIELDS)),
        NAMES.join(","),
    );
    let (get, set, for_each): (Function, Function, Function) = lua
        .load(include_str!("live.lua"))
        .set_name("=particles_live")
        .call((read, write, layout.0, layout.1, layout.2))
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

/// Run `f` on `id`'s emitter after carrying a `Local` one to where it stands,
/// with its frame; `None` without an emitter. The carry runs on an emitter
/// inactive in the hierarchy too (the sim's `carry_local_particles` skips it): a
/// particle `f` adds is placed under the current frame, so its siblings must be
/// under it as well, and the carry is idempotent, so reactivation moves them all
/// by what is left of the change.
fn with_carried<R>(
    scene: &mut Scene,
    id: u32,
    f: impl FnOnce(&mut ParticleEmitterComponent, &EmitterFrame) -> R,
) -> Option<R> {
    let frame = scene.world.emitter_frame(id);
    let mut emitter = scene.world.particles_mut(id)?;
    emitter.carry_to(frame);
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

/// A `Gpu` emitter's particles are the renderer's (#2170): a write changes
/// nothing, warns the first time, and returns the number live.
fn refuse(e: &mut ParticleEmitterComponent, verb: &str, id: u32) -> u32 {
    if e.runtime.gpu.first_warning() {
        log::warn!(
            "[Particles] {verb}: entity {id} simulates on the GPU; its particles cannot \
             be written (the call does nothing)"
        );
    }
    e.live_count() as u32
}

/// Replace `e`'s live particles, under its frame `frame`, with `records` (a
/// whole number of [`RECORD`]s): dead ones and those past `max_particles`
/// dropped, no death sub-emitter fired. A record whose source is still the live
/// particle it was read from (same index, same age) keeps that particle's hidden
/// fields (`start_frame`, `axis`, `turn`), wherever it now sits in the list; any
/// other is an added particle. Returns the number live.
fn write_records(e: &mut ParticleEmitterComponent, frame: &EmitterFrame, records: &[u8]) -> u32 {
    let cap = e.max_particles as usize;
    let mut live = Vec::with_capacity((records.len() / RECORD).min(cap));
    for r in records.chunks_exact(RECORD) {
        if live.len() == cap {
            break;
        }
        let src = u32::from_le_bytes(word(r, 0));
        let stamp = f32::from_le_bytes(word(r, 4));
        let mut values = [0.0f32; FIELDS];
        for (k, v) in values.iter_mut().enumerate() {
            *v = f32::from_le_bytes(word(r, 8 + k * 4));
        }
        let old = (src != NO_SOURCE)
            .then(|| e.runtime.particles.get(src as usize).copied())
            .flatten()
            .filter(|o| o.age.to_bits() == stamp.to_bits());
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
