//! src/api/material/surface.rs — the table-driven material fields: a
//! `Material.Set<api>(id, value)` / `Material.Get<api>(id)` pair for every row of
//! `authoring::material::material_fields` — the transparent-surface fields (#1523:
//! `SetBlendMode`, `GetReceiveShadows`, …) and the map coordinates (#1573:
//! `SetTiling`, `SetUvMapping`, …).
//!
//! A thin adapter (#287): it turns the Lua value into the row's `SurfaceValue` and
//! hands it to the shared `set_surface` op, which owns the validation the Inspector
//! and `DefineAsset` share. A value the field cannot take is a Lua error naming
//! what it takes.

use std::cell::RefCell;

use mlua::{IntoLua, Value};

use super::{put, with_material, Reg};
use crate::api::entity_arg::EntityArg;
use crate::scene::authoring::material::{
    self as mat_ops, material_fields, SurfaceField, SurfaceKind, SurfaceValue,
};
use crate::scene::Scene;

/// Register a Set/Get pair per surface field onto the `Material` `table`.
pub fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    for field in material_fields() {
        let set = format!("Set{}", field.api);
        let verb = set.clone();
        let f = scope.create_function(move |_, (id, value): (EntityArg, Value)| {
            let id = id.live("Material", &verb)?;
            let value = from_lua(field, &value).map_err(mlua::Error::RuntimeError)?;
            let mut result = Ok(());
            with_material(scene, id, |m, key| {
                result = mat_ops::set_surface(m, key, field, value);
            });
            result.map_err(mlua::Error::RuntimeError)
        });
        put(table, &set, f)?;

        let get = format!("Get{}", field.api);
        let verb = get.clone();
        let f = scope.create_function(move |lua, id: EntityArg| {
            let id = id.live("Material", &verb)?;
            let scene = scene.borrow();
            let key = scene.world.material(id).map(|m| m.first().to_string());
            match key.and_then(|k| scene.materials.get(&k)) {
                Some(m) => to_lua(lua, (field.get)(m)),
                None => Ok(Value::Nil),
            }
        });
        put(table, &get, f)?;
    }
    Ok(())
}

/// `value` as `field`'s kind takes it; any other Lua type is an error naming the
/// field. The row's own `check` still clamps and parses it.
pub(super) fn from_lua(field: &SurfaceField, value: &Value) -> Result<SurfaceValue, String> {
    let wrong = || format!("{}: unexpected {}", field.key, value.type_name());
    let number = |v: &Value| match v {
        Value::Number(n) => Some(*n as f32),
        Value::Integer(n) => Some(*n as f32),
        _ => None,
    };
    match (&field.kind, value) {
        (SurfaceKind::Choice(_), Value::String(s)) => {
            let name = s.to_str().map_err(|e| e.to_string())?;
            Ok(SurfaceValue::Text(name.to_string()))
        }
        (SurfaceKind::Toggle, Value::Boolean(b)) => Ok(SurfaceValue::Bool(*b)),
        (SurfaceKind::Number(_), v) => number(v).map(SurfaceValue::Number).ok_or_else(wrong),
        (SurfaceKind::Color, Value::Table(t)) => {
            let channel = |i| t.get::<Value>(i).ok().as_ref().and_then(number);
            match (channel(1), channel(2), channel(3)) {
                (Some(r), Some(g), Some(b)) => Ok(SurfaceValue::Color([r, g, b])),
                _ => Err(wrong()),
            }
        }
        // `{x, y}` by position, or `{x = …, y = …}`.
        (SurfaceKind::Vec2(_), Value::Table(t)) => {
            let lane = |i: usize, name: &str| {
                let v = t.get::<Value>(i).ok().filter(|v| !v.is_nil());
                v.or_else(|| t.get::<Value>(name).ok())
                    .as_ref()
                    .and_then(number)
            };
            match (lane(1, "x"), lane(2, "y")) {
                (Some(x), Some(y)) => Ok(SurfaceValue::Vec2([x, y])),
                _ => Err(wrong()),
            }
        }
        _ => Err(wrong()),
    }
}

/// A field's value back to Lua: a string, a boolean, a number, an `{r, g, b}` or
/// an `{x, y}`.
fn to_lua(lua: &mlua::Lua, value: SurfaceValue) -> mlua::Result<Value> {
    match value {
        SurfaceValue::Text(t) => t.into_lua(lua),
        SurfaceValue::Bool(b) => Ok(Value::Boolean(b)),
        SurfaceValue::Number(n) => Ok(Value::Number(f64::from(n))),
        SurfaceValue::Color(c) => c.into_lua(lua),
        SurfaceValue::Vec2(v) => v.into_lua(lua),
    }
}

#[cfg(test)]
#[path = "surface_tests.rs"]
mod tests;
