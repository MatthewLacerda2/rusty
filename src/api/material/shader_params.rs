//! src/api/material/shader_params.rs — `Material.SetShaderParam` /
//! `Material.GetShaderParam` (#399): a material's runtime shader params, Unity's
//! `material.SetFloat`.
//!
//! Thin adapters over `authoring::material::{set_shader_param, shader_param}`: they
//! resolve the entity's material, read its shader's param layout, and convert the
//! Lua value (a number, or an array for a vector param). Strict: a param the shader
//! does not expose at runtime, or a wrong number of values, raises an error naming
//! it and listing the runtime params there are.

use std::cell::RefCell;

use mlua::{IntoLua, Value};

use super::{put, Reg};
use crate::scene::authoring::material as mat_ops;
use crate::scene::Scene;

/// Register `SetShaderParam` / `GetShaderParam` onto the `Material` `table`.
pub fn register<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetShaderParam",
        scope.create_function(|_, (id, name, value): (u32, String, Value)| {
            let value = floats(value)?;
            let mut scene = scene.borrow_mut();
            let key = material_key(&mut scene, id)?;
            let layout = mat_ops::shader_layout(&scene.materials, &key).map_err(err)?;
            mat_ops::set_shader_param(&mut scene.materials, &key, &layout, &name, value)
                .map_err(err)
        }),
    )?;

    put(
        table,
        "GetShaderParam",
        scope.create_function(|lua, (id, name): (u32, String)| {
            let mut scene = scene.borrow_mut();
            let key = material_key(&mut scene, id)?;
            let layout = mat_ops::shader_layout(&scene.materials, &key).map_err(err)?;
            let v = mat_ops::shader_param(&scene.materials, &key, &layout, &name).map_err(err)?;
            match v[..] {
                [x] => x.into_lua(lua),
                _ => v.into_lua(lua),
            }
        }),
    )
}

/// Entity `id`'s material key (created if it has none, like every `Material` setter).
fn material_key(scene: &mut Scene, id: u32) -> mlua::Result<String> {
    mat_ops::ensure_material_key(scene, id).ok_or_else(|| err(format!("no entity {id}")))
}

/// A Lua number, or an array of numbers, as floats.
fn floats(value: Value) -> mlua::Result<Vec<f32>> {
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
