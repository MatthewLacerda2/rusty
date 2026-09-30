//! src/api/random.rs — `Random` namespace (#443).
//!
//! Unity's `UnityEngine.Random` subset, backed by the seeded
//! [`Random`](crate::core::random::Random) resource the script runtime owns.
//! `Value` / `Range` / `InsideUnitSphere` / `OnUnitSphere` / `InsideUnitCircle` /
//! `SetSeed`. The sandboxed `math.random` / `math.randomseed` (installed by the
//! scripting layer) route through this namespace, so there is exactly one stream.

use std::cell::RefCell;

use mlua::{Lua, Value};

use super::{put, Reg};
use crate::core::random::Random;

/// Register the `Random` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    rng: &'scope RefCell<Random>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;
    put(
        &table,
        "Value",
        scope.create_function(|_, ()| Ok(rng.borrow_mut().value())),
    )?;
    put(
        &table,
        "Range",
        scope.create_function(|_, (min, max): (Value, Value)| range(rng, min, max)),
    )?;
    put(
        &table,
        "InsideUnitSphere",
        scope.create_function(|_, ()| {
            let v = rng.borrow_mut().inside_unit_sphere();
            Ok((v.x, v.y, v.z))
        }),
    )?;
    put(
        &table,
        "OnUnitSphere",
        scope.create_function(|_, ()| {
            let v = rng.borrow_mut().on_unit_sphere();
            Ok((v.x, v.y, v.z))
        }),
    )?;
    put(
        &table,
        "InsideUnitCircle",
        scope.create_function(|_, ()| {
            let v = rng.borrow_mut().inside_unit_circle();
            Ok((v.x, v.y))
        }),
    )?;
    put(
        &table,
        "SetSeed",
        scope.create_function(|_, seed: i64| {
            rng.borrow_mut().set_seed(seed as u64);
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
fn range<'lua>(
    rng: &RefCell<Random>,
    min: Value<'lua>,
    max: Value<'lua>,
) -> mlua::Result<Value<'lua>> {
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
