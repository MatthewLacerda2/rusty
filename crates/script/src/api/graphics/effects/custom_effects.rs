//! `Graphics.{Get,Set}CustomEffects` (#397): the ordered authored post-FX modules the
//! active post-processing volume runs, through the same
//! `authoring::rendering::post_processing` op the Inspector's list calls — and
//! `Graphics.{Get,Set}PostParam` (#671), those modules' runtime params.

use mlua::{IntoLua, Value};

use super::super::super::material::shader::params::floats;
use super::super::super::{put, Reg};
use super::super::state::{with_vc, with_vc_mut};
use crate::api::ApiCell;
use rusty_scene::authoring::rendering::post_processing as vc_ops;
use rusty_scene::Scene;
use rusty_shadergen::post_params::catalog_default;

/// The list and its params.
pub(super) fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    register_names(lua, table, scene)?;
    register_params(lua, table, scene)
}

/// `SetCustomEffects({names})` replaces the list (an invalid name is a Lua error and
/// nothing changes); `GetCustomEffects()` returns it, empty without an active volume.
fn register_names(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetCustomEffects",
        lua.create_function(move |lua, names: Vec<String>| {
            let mut result = Ok(());
            with_vc_mut(&*scene.get(lua)?, |e| {
                result = vc_ops::custom_effects::set_names(e, names)
            });
            result.map_err(mlua::Error::RuntimeError)
        }),
    )?;
    put(
        table,
        "GetCustomEffects",
        lua.create_function(move |lua, ()| {
            let names = with_vc(&*scene.get(lua)?, |vc| vc.custom_effects.names.clone())
                .unwrap_or_default();
            lua.create_sequence_from(names)
        }),
    )
}

/// `SetPostParam(name, value)` sets a runtime param of the active volume's effects
/// (strict: an unknown or baked name, a wrong arity, or no active volume is a Lua
/// error); `GetPostParam(name)` reads it back, or the catalog default with no volume.
fn register_params(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetPostParam",
        lua.create_function(move |lua, (name, value): (String, Value)| {
            let value = floats(value)?;
            let mut result = Err("SetPostParam: no active post-processing volume".to_owned());
            with_vc_mut(&*scene.get(lua)?, |e| {
                result = vc_ops::custom_effects::set_param(e, &name, value)
            });
            result.map_err(mlua::Error::RuntimeError)
        }),
    )?;
    put(
        table,
        "GetPostParam",
        lua.create_function(move |lua, name: String| {
            let v = with_vc(&*scene.get(lua)?, |vc| {
                vc_ops::custom_effects::param(vc, &name)
            })
            .unwrap_or_else(|| catalog_default(&name))
            .map_err(mlua::Error::RuntimeError)?;
            match v[..] {
                [x] => x.into_lua(lua),
                _ => v.into_lua(lua),
            }
        }),
    )
}

#[cfg(test)]
#[path = "custom_effects_tests.rs"]
mod post_params_tests;
