//! crates/script/src/api/particle/arrays.rs — whole-array access to the live
//! particles (#2233): `GetArrays` hands a script one plain array of numbers per
//! field it names, `SetArrays` writes them back. A rule over every particle is
//! then a loop over arrays inside one Lua function, with no call, table or
//! string per particle: Unity's job-system access (`ParticleSystemJobData`),
//! for the same reason.
//!
//! The arrays are index-aligned with the live particles, oldest first, so a
//! write keeps each particle's hidden fields. They are tied to the read: the
//! table carries the ages the particles had (in a metatable a script cannot
//! reach), and a write to an emitter whose particles are no longer those (a
//! tick aged them; an emit, a kill or another write added, removed or aged one)
//! is an error that changes nothing.

use mlua::{AnyUserData, Lua, Table, Value};

use super::live::{apply, fields, with_carried, FIELDS, NAMES};
use crate::api::entity_arg::EntityArg;
use crate::api::{ApiCell, Reg};
use rusty_components::particle::Particle;
use rusty_scene::Scene;

/// What a `GetArrays` table was read from: its emitter and each particle's age,
/// bit for bit.
struct Stamp {
    id: u32,
    ages: Vec<u32>,
}

/// The metatable key holding a table's [`Stamp`].
const STAMP: &str = "__particles_read";

/// Register `GetArrays` and `SetArrays` onto the `Particles` table.
pub(super) fn register(lua: &Lua, table: &Table, scene: ApiCell<Scene>) -> Reg {
    let get = lua
        .create_function(move |lua, (id, names): (EntityArg, Value)| {
            let slots = slots("GetArrays", names)?;
            let scene = &*scene.get(lua)?;
            let id = id.read();
            let live = with_carried(&mut scene.borrow_mut(), id, |e, _| {
                e.runtime.particles.clone()
            })
            .unwrap_or_default();
            columns(lua, id, &live, &slots)
        })
        .map_err(|e| e.to_string())?;
    let set = lua
        .create_function(move |lua, (id, arrays): (EntityArg, Value)| {
            let id = id.live("Particles", "SetArrays")?;
            let Value::Table(arrays) = arrays else {
                return Err(error("SetArrays: the arrays are not a table".into()));
            };
            let scene = &*scene.get(lua)?;
            let live = set_arrays(&mut scene.borrow_mut(), id, &arrays);
            live
        })
        .map_err(|e| e.to_string())?;
    table.set("GetArrays", get).map_err(|e| e.to_string())?;
    table.set("SetArrays", set).map_err(|e| e.to_string())?;
    Ok(())
}

/// `SetArrays(id, arrays)` on `scene`: the number live after, or an error that
/// changed nothing.
fn set_arrays(scene: &mut Scene, id: u32, arrays: &Table) -> mlua::Result<u32> {
    let stamp = arrays
        .metatable()
        .and_then(|m| m.raw_get::<AnyUserData>(STAMP).ok())
        .ok_or_else(|| error("SetArrays: the arrays are not a GetArrays table".into()))?;
    let stamp = stamp.borrow::<Stamp>()?;
    if stamp.id != id {
        return Err(error(format!(
            "SetArrays: the arrays were read from entity {}, not {id}",
            stamp.id
        )));
    }
    let written = read_back(arrays, stamp.ages.len())?;
    let live = with_carried(scene, id, |e, _| {
        let ps = &mut e.runtime.particles;
        let same = ps.len() == stamp.ages.len()
            && ps
                .iter()
                .zip(&stamp.ages)
                .all(|(p, &a)| p.age.to_bits() == a);
        if !same {
            return Err(error(
                "SetArrays: the arrays are from an older read (the particles have ticked, or \
                 been added, removed or aged since); read and write in the same function"
                    .into(),
            ));
        }
        write(ps, &written);
        Ok(ps.len() as u32)
    });
    live.unwrap_or(Ok(0))
}

/// A `Particles.SetArrays` / `GetArrays` error.
fn error(what: String) -> mlua::Error {
    mlua::Error::RuntimeError(format!("Particles.{what}"))
}

/// `v`'s type as Lua's `type` names it.
fn kind(v: &Value) -> &'static str {
    match v {
        Value::Integer(_) => "number",
        v => v.type_name(),
    }
}

/// The wire slots `names` (an array of field names) asks for, in its order.
fn slots(verb: &str, names: Value) -> mlua::Result<Vec<usize>> {
    let Value::Table(names) = names else {
        return Err(error(format!(
            "{verb}: the fields are a {}, not an array of names",
            kind(&names)
        )));
    };
    let mut slots = Vec::new();
    for name in names.sequence_values::<Value>() {
        let name = name?;
        let slot = name.as_string().and_then(|s| {
            s.to_str()
                .ok()
                .and_then(|s| NAMES.iter().position(|n| *n == &*s))
        });
        let Some(slot) = slot else {
            return Err(error(format!(
                "{verb}: {} is not a particle field (one of {})",
                name.to_string().unwrap_or_else(|_| kind(&name).into()),
                NAMES.join(", ")
            )));
        };
        slots.push(slot);
    }
    Ok(slots)
}

/// The table `GetArrays` returns: one array per slot of `slots`, keyed by its
/// name, stamped with `live`'s ages.
fn columns(lua: &Lua, id: u32, live: &[Particle], slots: &[usize]) -> mlua::Result<Table> {
    let out = lua.create_table_with_capacity(0, slots.len())?;
    let rows: Vec<[f32; FIELDS]> = live.iter().map(fields).collect();
    for &slot in slots {
        let column = lua.create_sequence_from(rows.iter().map(|r| f64::from(r[slot])))?;
        out.raw_set(NAMES[slot], column)?;
    }
    let ages = live.iter().map(|p| p.age.to_bits()).collect();
    let meta = lua.create_table()?;
    meta.raw_set(STAMP, lua.create_any_userdata(Stamp { id, ages })?)?;
    meta.raw_set("__metatable", false)?;
    out.set_metatable(Some(meta))?;
    Ok(out)
}

/// Each field array `arrays` holds, as `(slot, values)`, checked: a whole array
/// of `count` numbers, or an error naming the field and the entry.
fn read_back(arrays: &Table, count: usize) -> mlua::Result<Vec<(usize, Vec<f32>)>> {
    let mut written = Vec::new();
    for (slot, name) in NAMES.iter().enumerate() {
        let column = match arrays.raw_get::<Value>(*name)? {
            Value::Nil => continue,
            Value::Table(t) => t,
            other => {
                return Err(error(format!(
                    "SetArrays: {name} is a {}, not an array",
                    kind(&other)
                )))
            }
        };
        if column.raw_len() != count {
            return Err(error(format!(
                "SetArrays: {name} holds {} entries, not the {count} read",
                column.raw_len()
            )));
        }
        let mut values = Vec::with_capacity(count);
        for i in 1..=count {
            match column.raw_get::<Value>(i)? {
                Value::Number(x) => values.push(x as f32),
                Value::Integer(x) => values.push(x as f32),
                other => {
                    return Err(error(format!(
                        "SetArrays: {name}[{i}] is a {}, not a number",
                        kind(&other)
                    )))
                }
            }
        }
        written.push((slot, values));
    }
    Ok(written)
}

/// Write `written` into `ps` (index-aligned) and drop the particles dead after.
fn write(ps: &mut Vec<Particle>, written: &[(usize, Vec<f32>)]) {
    for (i, p) in ps.iter_mut().enumerate() {
        let mut values = [f32::NAN; FIELDS];
        for (slot, column) in written {
            values[*slot] = column[i];
        }
        apply(p, &values);
    }
    ps.retain(|p| p.age < p.lifetime);
}
