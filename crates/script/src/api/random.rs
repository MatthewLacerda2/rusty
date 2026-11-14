//! crates/script/src/api/random.rs — `Random` namespace (#443).
//!
//! Unity's `UnityEngine.Random` subset, backed by the seeded
//! [`Random`] resource the script runtime owns.
//! `Value` / `Range` / `InsideUnitSphere` / `OnUnitSphere` / `InsideUnitCircle` /
//! `SetSeed`. The sandboxed `math.random` / `math.randomseed` (installed by the
//! scripting layer) route through this namespace, so there is exactly one stream.

use std::cell::RefCell;

use mlua::{Lua, Value};

use super::{put, Reg};
use crate::api::ApiCell;
use rusty_core::random::Random;

/// Register the `Random` namespace onto `lua`.
pub fn register(lua: &Lua, rng: ApiCell<Random>) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    put(
        &table,
        "Value",
        lua.create_function(move |lua, ()| Ok(rng.get(lua)?.borrow_mut().value())),
    )?;
    put(
        &table,
        "Range",
        lua.create_function(move |lua, (min, max): (Value, Value)| {
            range(&*rng.get(lua)?, min, max)
        }),
    )?;
    put(
        &table,
        "InsideUnitSphere",
        lua.create_function(move |lua, ()| {
            let v = rng.get(lua)?.borrow_mut().inside_unit_sphere();
            Ok((v.x, v.y, v.z))
        }),
    )?;
    put(
        &table,
        "OnUnitSphere",
        lua.create_function(move |lua, ()| {
            let v = rng.get(lua)?.borrow_mut().on_unit_sphere();
            Ok((v.x, v.y, v.z))
        }),
    )?;
    put(
        &table,
        "InsideUnitCircle",
        lua.create_function(move |lua, ()| {
            let v = rng.get(lua)?.borrow_mut().inside_unit_circle();
            Ok((v.x, v.y))
        }),
    )?;
    put(
        &table,
        "SetSeed",
        lua.create_function(move |lua, seed: i64| {
            rng.get(lua)?.borrow_mut().set_seed(seed as u64);
            Ok(())
        }),
    )?;
    lua.globals()
        .set("Random", table)
        .map_err(|e| e.to_string())
}

/// `Random.Range(min, max)`: two Lua **integers** draw an integer in
/// `[min, max)` (max exclusive); any float argument draws a float in
/// `[min, max)`. Unity's two overloads, told apart by Lua's number subtype.
fn range(rng: &RefCell<Random>, min: Value, max: Value) -> mlua::Result<Value> {
    match (&min, &max) {
        (Value::Integer(a), Value::Integer(b)) => {
            Ok(Value::Integer(rng.borrow_mut().range_i64(*a, *b)))
        }
        _ => {
            let (a, b) = (number(&min)?, number(&max)?);
            Ok(Value::Number(rng.borrow_mut().range_f64(a, b)))
        }
    }
}

fn number(v: &Value) -> mlua::Result<f64> {
    match v {
        Value::Integer(i) => Ok(*i as f64),
        Value::Number(n) => Ok(*n),
        other => Err(mlua::Error::RuntimeError(format!(
            "Random.Range expects numbers, got {}",
            other.type_name()
        ))),
    }
}
