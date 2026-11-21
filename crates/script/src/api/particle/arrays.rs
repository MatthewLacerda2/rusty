//! Arrays are index-aligned with the live particles, oldest first. They are tied to the read: a write after anything bumped the emitter's revision is an error that changes nothing. A `Gpu` emitter's particles are the renderer's: `GetArrays` reads empty, `SetArrays` changes nothing and warns once.

use mlua::{AnyUserData, Lua, Table, Value};

use super::gpu::refuse;
use super::live::{apply, field, revision, touched, with_carried, FIELDS, NAMES};
use crate::api::entity_arg::EntityArg;
use crate::api::{ApiCell, Reg};
use rusty_components::particle::Particle;
use rusty_scene::Scene;

struct Stamp {
    id: u32,
    revision: i64,
    count: usize,
}

const STAMP: &str = "__particles_read";

pub(super) fn register(lua: &Lua, table: &Table, scene: ApiCell<Scene>) -> Reg {
    let get = lua
        .create_function(move |lua, (id, names): (EntityArg, Value)| {
            let slots = slots("GetArrays", names)?;
            let scene = &*scene.get(lua)?;
            let id = id.read();
            let mut scene = scene.borrow_mut();
            let read = with_carried(&mut scene, id, |e, _| {
                let stamp = Stamp {
                    id,
                    revision: revision(e),
                    count: e.runtime.particles.len(),
                };
                columns(lua, &e.runtime.particles, &slots, stamp)
            });
            let stamp = Stamp {
                id,
                revision: -1,
                count: 0,
            };
            read.unwrap_or_else(|| columns(lua, &[], &slots, stamp))
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
    let written = read_back(arrays, stamp.count)?;
    let live = with_carried(scene, id, |e, _| {
        if e.on_gpu() {
            return Ok(refuse(e, "SetArrays", id));
        }
        if revision(e) != stamp.revision || e.runtime.particles.len() != stamp.count {
            return Err(error(
                "SetArrays: the arrays are from an older read (the particles have ticked, or \
                 been emitted, cleared or written since); read and write in the same function"
                    .into(),
            ));
        }
        write(&mut e.runtime.particles, &written);
        touched(e);
        Ok(e.runtime.particles.len() as u32)
    });
    live.unwrap_or(Ok(0))
}

fn error(what: String) -> mlua::Error {
    mlua::Error::RuntimeError(format!("Particles.{what}"))
}

fn kind(v: &Value) -> &'static str {
    match v {
        Value::Integer(_) => "number",
        v => v.type_name(),
    }
}

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

fn columns(lua: &Lua, live: &[Particle], slots: &[usize], stamp: Stamp) -> mlua::Result<Table> {
    let out = lua.create_table_with_capacity(0, slots.len())?;
    for &slot in slots {
        let column = lua.create_sequence_from(live.iter().map(|p| f64::from(field(p, slot))))?;
        out.raw_set(NAMES[slot], column)?;
    }
    let meta = lua.create_table()?;
    meta.raw_set(STAMP, lua.create_any_userdata(stamp)?)?;
    meta.raw_set("__metatable", false)?;
    out.set_metatable(Some(meta))?;
    Ok(out)
}

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
