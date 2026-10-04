//! src/api/material/shader_params.rs — runtime shader params from Lua (#399, #670).
//!
//! - `SetShaderParam(id, name, v)` / `GetShaderParam(id, name)` /
//!   `ClearShaderParam(id [, name])` — entity `id`'s **override** (#670, Unity's
//!   `renderer.material` / `MaterialPropertyBlock`): it wins over the material's value
//!   for that entity alone, and is runtime-only. `Get` reads the override, else the
//!   material's value, else the baked default.
//! - `SetAssetShaderParam(asset, name, v)` / `GetAssetShaderParam(asset, name)` — the
//!   shared value on the library asset (Unity's `sharedMaterial.SetFloat`): every
//!   entity using it that has no override sees it, and it saves with the scene.
//!
//! Thin adapters over `authoring::material`'s shader-param ops, which validate once:
//! a param the shader does not expose at runtime, or a wrong number of values,
//! raises an error naming it and listing the runtime params there are.

use std::cell::RefCell;

use mlua::{IntoLua, Lua, Value};

use super::{put, Reg};
use crate::scene::authoring::material as mat_ops;
use crate::scene::Scene;

/// Register the shader-param verbs onto the `Material` `table`.
pub fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetShaderParam",
        scope.create_function(|_, (id, name, value): (u32, String, Value)| {
            let value = floats(value)?;
            mat_ops::set_entity_shader_param(&mut scene.borrow_mut(), id, &name, value).map_err(err)
        }),
    )?;
    put(
        table,
        "GetShaderParam",
        scope.create_function(|lua, (id, name): (u32, String)| {
            let v = mat_ops::entity_shader_param(&mut scene.borrow_mut(), id, &name);
            to_lua(lua, v.map_err(err)?)
        }),
    )?;
    put(
        table,
        "ClearShaderParam",
        scope.create_function(|_, (id, name): (u32, Option<String>)| {
            let mut scene = scene.borrow_mut();
            mat_ops::clear_entity_shader_param(&mut scene, id, name.as_deref()).map_err(err)
        }),
    )?;
    put(
        table,
        "SetAssetShaderParam",
        scope.create_function(|_, (asset, name, value): (String, String, Value)| {
            let value = floats(value)?;
            let mut scene = scene.borrow_mut();
            let layout = asset_layout(&scene, &asset)?;
            mat_ops::set_shader_param(&mut scene.materials, &asset, &layout, &name, value)
                .map_err(err)
        }),
    )?;
    put(
        table,
        "GetAssetShaderParam",
        scope.create_function(|lua, (asset, name): (String, String)| {
            let scene = scene.borrow();
            let layout = asset_layout(&scene, &asset)?;
            let v = mat_ops::shader_param(&scene.materials, &asset, &layout, &name);
            to_lua(lua, v.map_err(err)?)
        }),
    )
}

/// Library asset `asset`'s shader layout; an absent asset is an error naming it.
fn asset_layout(scene: &Scene, asset: &str) -> mlua::Result<crate::shadergen::params::ParamLayout> {
    if !scene.materials.contains_key(asset) {
        return Err(err(format!("no material asset {asset:?}")));
    }
    mat_ops::shader_layout(&scene.materials, asset).map_err(err)
}

/// A param value as Lua sees it: a number for one lane, else an array.
fn to_lua(lua: &Lua, v: Vec<f32>) -> mlua::Result<Value> {
    match v[..] {
        [x] => x.into_lua(lua),
        _ => v.into_lua(lua),
    }
}

/// A Lua number, or an array of numbers, as floats.
pub(crate) fn floats(value: Value) -> mlua::Result<Vec<f32>> {
    match value {
        Value::Integer(i) => Ok(vec![i as f32]),
        Value::Number(n) => Ok(vec![n as f32]),
        Value::Table(t) => t.sequence_values::<f32>().collect(),
        other => Err(err(format!(
            "a shader param value is a number or an array of numbers, got {}",
            other.type_name()
        ))),
    }
}

fn err(message: String) -> mlua::Error {
    mlua::Error::RuntimeError(message)
}

#[cfg(test)]
#[path = "shader_params_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "shader_overrides_tests.rs"]
mod overrides_tests;
