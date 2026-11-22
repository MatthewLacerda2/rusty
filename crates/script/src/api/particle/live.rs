//! A field written back as NaN or infinity keeps its value; an added particle's is `ParticleEmitterComponent::placed`'s.

use mlua::{Function, Lua, Table};

use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::api::Reg;
use rusty_components::particle::{EmitterFrame, Particle, ParticleEmitterComponent};
use rusty_scene::Scene;

/// `live.lua` refuses to load if its own order differs, so this list is the layout's one source.
pub(super) const NAMES: [&str; FIELDS] = [
    "x", "y", "z", "vx", "vy", "vz", "age", "lifetime", "size", "rotation", "r", "g", "b", "a",
    "custom1", "custom2", "custom3", "custom4",
];
pub(super) const FIELDS: usize = 18;
const PARTICLE: usize = FIELDS * 4;
const RECORD: usize = 4 + PARTICLE;
const NO_SOURCE: u32 = u32::MAX;
const SLOTS: u32 = 32;
const _: () = assert!(FIELDS as u32 <= SLOTS, "an edit's slot must fit its stride");

struct Snapshot(Vec<Particle>);

const HALVES: [&str; 4] = ["GetParticles", "SetParticles", "ForEach", "ForEach/edit"];

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

pub(super) fn touched(e: &mut ParticleEmitterComponent) {
    e.runtime.revision = e.runtime.revision.wrapping_add(1);
}

pub(super) fn revision(e: &ParticleEmitterComponent) -> i64 {
    e.runtime.revision as i64
}

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

/// A `Local` emitter is carried to where it stands first, even inactive in the hierarchy: a particle added is placed under the current frame, and the carry is idempotent.
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

fn packed(particles: &[Particle]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(particles.len() * PARTICLE);
    for p in particles {
        for x in fields(p) {
            bytes.extend_from_slice(&x.to_le_bytes());
        }
    }
    bytes
}

fn word(b: &[u8], at: usize) -> [u8; 4] {
    [b[at], b[at + 1], b[at + 2], b[at + 3]]
}

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

pub(super) fn fields(p: &Particle) -> [f32; FIELDS] {
    let ([px, py, pz], [vx, vy, vz]) = (p.position.to_array(), p.velocity.to_array());
    let ([r, g, b, a], [c1, c2, c3, c4]) = (p.color, p.custom);
    let rotation = p.rotation.to_degrees();
    [
        px, py, pz, vx, vy, vz, p.age, p.lifetime, p.size, rotation, r, g, b, a, c1, c2, c3, c4,
    ]
}

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

/// Rotation is written only when it differs from `p`'s in degrees, so a round trip keeps the radians bit for bit.
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
