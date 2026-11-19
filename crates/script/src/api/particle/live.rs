//! crates/script/src/api/particle/live.rs — the live-particle calls (#2171):
//! `GetParticles` / `SetParticles`, Unity's `ParticleSystem.GetParticles` /
//! `SetParticles`, and `ForEach`, one reused table refilled per particle.
//!
//! A particle table is flat and in world space: `x, y, z`, `vx, vy, vz`, `age`,
//! `lifetime`, `size` (start size), `rotation` (degrees), `r, g, b, a` (the start
//! tint) and `custom1` to `custom4` (four numbers the engine never reads). The
//! tables are built and read by `live.lua`; this half moves the particles across
//! as one packed string each way (`read`, `write`), since a Rust call per field
//! costs several times the whole Lua side, and takes `ForEach`'s changes back as
//! a list of edits (`edit`) applied to what it read. Every write bumps the
//! emitter's `revision` ([`touched`]), as a tick, an emit and a clear do. A field written back as a
//! NaN or an infinity (what `live.lua` sends for one left out) keeps its value;
//! an added particle's is `ParticleEmitterComponent::placed`'s. A table keeps
//! the identity of the particle it was read from (`live.lua` sends its source
//! with it, and the revision of its read), so a script that removes or reorders
//! entries keeps each survivor's hidden fields; after anything else changed the
//! emitter, every entry is an added particle. A `Local` emitter is carried to where it stands before every
//! read and write, so what a script reads and writes is where the particles draw
//! now. The fastest access, whole arrays per field, is `arrays.rs`. A `Gpu`
//! emitter's particles are the renderer's: its guard is `gpu.rs`.

use mlua::{Function, Lua, Table};

use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::api::Reg;
use rusty_components::particle::{EmitterFrame, Particle, ParticleEmitterComponent};
use rusty_scene::Scene;

/// The f32s of one particle on the wire, named in wire order. `live.lua` gets
/// the names and the two formats below as upvalues and refuses to load if its
/// own order differs, so this list is the layout's one source.
pub(super) const NAMES: [&str; FIELDS] = [
    "x", "y", "z", "vx", "vy", "vz", "age", "lifetime", "size", "rotation", "r", "g", "b", "a",
    "custom1", "custom2", "custom3", "custom4",
];
pub(super) const FIELDS: usize = 18;
/// The bytes of one particle on the wire, and of one record written back: a u32
/// source (the live particle a table was read from, [`NO_SOURCE`] for one the
/// script built) first.
const PARTICLE: usize = FIELDS * 4;
const RECORD: usize = 4 + PARTICLE;
/// A record's source when the table did not come from the emitter's newest read.
const NO_SOURCE: u32 = u32::MAX;
/// An edit's key is `index * SLOTS + slot`; `live.lua` gets it as an upvalue.
const SLOTS: u32 = 32;
const _: () = assert!(FIELDS as u32 <= SLOTS, "an edit's slot must fit its stride");

/// The particles a `ForEach` read, held until its edits come back: the write
/// replaces what is live with them, as `SetParticles` replaces it with its list.
struct Snapshot(Vec<Particle>);

/// The verbs (the first three) and the Rust halves' keys in `RUST_HALVES`.
const HALVES: [&str; 4] = ["GetParticles", "SetParticles", "ForEach", "ForEach/edit"];

/// Register `GetParticles`, `SetParticles` and `ForEach` onto the `Particles` table.
pub(super) fn register(lua: &Lua, table: &Table, scene: ApiCell<Scene>) -> Reg {
    let halves = (read(lua, scene), write(lua, scene), edit(lua, scene));
    let (read, write, edit) = match halves {
        (Ok(r), Ok(w), Ok(e)) => (r, w, e),
        (Err(e), ..) | (_, Err(e), _) | (.., Err(e)) => return Err(e.to_string()),
    };
    let layout = (
        format!("<{}", "f".repeat(FIELDS)),
        format!("<I4{}", "f".repeat(FIELDS)),
        NAMES.join(","),
    );
    let halves = HALVES.into_iter().zip([&read, &write, &read, &edit]);
    let halves = super::super::rust_halves(lua, "Particles", halves).map_err(|e| e.to_string())?;
    let (get, set, for_each): (Function, Function, Function) = lua
        .load(include_str!("live.lua"))
        .set_name("=particles_live")
        .call((halves, layout.0, layout.1, layout.2, SLOTS))
        .map_err(|e| e.to_string())?;
    for (name, f) in HALVES.into_iter().zip([get, set, for_each]) {
        table.set(name, f).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Mark `e`'s particles changed: a read before this is no longer of them.
pub(super) fn touched(e: &mut ParticleEmitterComponent) {
    e.runtime.revision = e.runtime.revision.wrapping_add(1);
}

/// `e`'s revision as a Lua integer (its bits; only equality is ever asked).
pub(super) fn revision(e: &ParticleEmitterComponent) -> i64 {
    e.runtime.revision as i64
}

/// `read(id, keep)`: the emitter's particles on the wire, their count, the
/// emitter's revision (`nil` without one) and, when `keep`, a [`Snapshot`] of
/// them for `edit`.
fn read(lua: &Lua, scene: ApiCell<Scene>) -> mlua::Result<Function> {
    lua.create_function(move |lua, (id, keep): (EntityArg, bool)| {
        let scene = &*scene.get(lua)?;
        let read = with_carried(&mut scene.borrow_mut(), id.read(), |e, _| {
            let live = &e.runtime.particles;
            let kept = keep.then(|| Snapshot(live.clone()));
            (packed(live), Some(revision(e)), kept)
        });
        let (bytes, rev, kept) = read.unwrap_or_default();
        let count = bytes.len() / PARTICLE;
        let kept = kept.map(|k| lua.create_any_userdata(k)).transpose()?;
        Ok((lua.create_string(bytes)?, count, rev, kept))
    })
}

/// `write(id, verb, rev, records)`: replace the live particles with `records`,
/// their sources honoured only when `rev` is still the emitter's revision.
fn write(lua: &Lua, scene: ApiCell<Scene>) -> mlua::Result<Function> {
    type Args = (EntityArg, String, Option<i64>, mlua::LuaString);
    lua.create_function(move |lua, (id, verb, rev, records): Args| {
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
        let live = with_carried(&mut scene.borrow_mut(), id, |e, frame| {
            if e.on_gpu() {
                return super::gpu::refuse(e, &verb, id);
            }
            let same = rev == Some(revision(e));
            write_records(e, frame, &records, same)
        });
        Ok(live.unwrap_or(0))
    })
}

/// `edit(id, snapshot, edits, kills)`: replace the live particles with the
/// `ForEach` read `snapshot`, edited ([`edited`]).
fn edit(lua: &Lua, scene: ApiCell<Scene>) -> mlua::Result<Function> {
    type Args = (EntityArg, Option<mlua::AnyUserData>, Vec<f64>, Vec<u32>);
    lua.create_function(move |lua, (id, kept, edits, kills): Args| {
        let id = id.live("Particles", "ForEach")?;
        let scene = &*scene.get(lua)?;
        let Some(kept) = kept else { return Ok(0) };
        let Snapshot(read) = kept.take::<Snapshot>()?;
        let edited = edited(read, &edits, &kills).map_err(mlua::Error::RuntimeError)?;
        let live = with_carried(&mut scene.borrow_mut(), id, |e, _| {
            let cap = e.max_particles as usize;
            e.runtime.particles = edited.into_iter().take(cap).collect();
            touched(e);
            e.live_count() as u32
        });
        Ok(live.unwrap_or(0))
    })
}

/// Run `f` on `id`'s emitter after carrying a `Local` one to where it stands,
/// with its frame; `None` without an emitter. The carry runs on an emitter
/// inactive in the hierarchy too (the sim's `carry_local_particles` skips it): a
/// particle `f` adds is placed under the current frame, so its siblings must be
/// under it as well, and the carry is idempotent, so reactivation moves them all
/// by what is left of the change.
pub(super) fn with_carried<R>(
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

/// `p`'s field at wire `slot`, as [`fields`] has it.
pub(super) fn field(p: &Particle, slot: usize) -> f32 {
    match slot {
        0..=2 => p.position[slot],
        3..=5 => p.velocity[slot - 3],
        6 => p.age,
        7 => p.lifetime,
        8 => p.size,
        9 => p.rotation.to_degrees(),
        10..=13 => p.color[slot - 10],
        _ => p.custom[slot - 14],
    }
}

/// `p`'s fields in wire order, its rotation in degrees.
pub(super) fn fields(p: &Particle) -> [f32; FIELDS] {
    let ([px, py, pz], [vx, vy, vz]) = (p.position.to_array(), p.velocity.to_array());
    let ([r, g, b, a], [c1, c2, c3, c4]) = (p.color, p.custom);
    let rotation = p.rotation.to_degrees();
    [
        px, py, pz, vx, vy, vz, p.age, p.lifetime, p.size, rotation, r, g, b, a, c1, c2, c3, c4,
    ]
}

/// Replace `e`'s live particles, under its frame `frame`, with `records` (a
/// whole number of [`RECORD`]s): dead ones and those past `max_particles`
/// dropped, no death sub-emitter fired. When `same` (nothing changed the
/// emitter since the read), a record with a source keeps that particle's hidden
/// fields (`start_frame`, `axis`, `turn`), wherever it now sits in the list; any
/// other is an added particle. Returns the number live.
fn write_records(
    e: &mut ParticleEmitterComponent,
    frame: &EmitterFrame,
    records: &[u8],
    same: bool,
) -> u32 {
    let cap = e.max_particles as usize;
    let mut live = Vec::with_capacity((records.len() / RECORD).min(cap));
    for r in records.chunks_exact(RECORD) {
        if live.len() == cap {
            break;
        }
        let src = u32::from_le_bytes(word(r, 0));
        let mut values = [0.0f32; FIELDS];
        for (k, v) in values.iter_mut().enumerate() {
            *v = f32::from_le_bytes(word(r, 4 + k * 4));
        }
        let old = (same && src != NO_SOURCE)
            .then(|| e.runtime.particles.get(src as usize).copied())
            .flatten();
        let mut p = old.unwrap_or_else(|| e.placed(frame));
        apply(&mut p, &values);
        if p.age < p.lifetime {
            live.push(p);
        }
    }
    e.runtime.particles = live;
    touched(e);
    e.runtime.particles.len() as u32
}

/// `read` with `edits` (`key, value` pairs, the key `index * SLOTS + slot`)
/// applied and the particles at `kills` and those dead after the edits
/// dropped; an error when a key is past the read (the two halves disagree).
fn edited(mut read: Vec<Particle>, edits: &[f64], kills: &[u32]) -> Result<Vec<Particle>, String> {
    for pair in edits.chunks_exact(2) {
        let key = pair[0] as u32;
        let (index, slot) = ((key / SLOTS) as usize, (key % SLOTS) as usize);
        let (Some(p), true) = (read.get_mut(index), slot < FIELDS) else {
            return Err(format!(
                "Particles.ForEach: edit {key} is past the {} particles read \
                 (live.lua and live.rs disagree on the layout)",
                read.len()
            ));
        };
        let mut values = [f32::NAN; FIELDS];
        values[slot] = pair[1] as f32;
        apply(p, &values);
    }
    let mut killed = vec![false; read.len()];
    for &k in kills {
        if let Some(slot) = killed.get_mut(k as usize) {
            *slot = true;
        }
    }
    let mut killed = killed.into_iter();
    read.retain(|p| !killed.next().unwrap_or(false) && p.age < p.lifetime);
    Ok(read)
}

/// Write `values` (wire order) into `p`; a NaN or an infinity keeps the field.
/// The rotation is written only when it differs from `p`'s in degrees, so a
/// round trip keeps the radians bit for bit.
pub(super) fn apply(p: &mut Particle, values: &[f32; FIELDS]) {
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
