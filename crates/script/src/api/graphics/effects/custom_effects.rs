use mlua::{IntoLua, Value};

use super::super::super::material::shader::params::floats;
use super::super::super::{put, Reg};
use super::super::state::{with_vc, with_vc_mut};
use crate::api::ApiCell;
use rusty_edit::authoring::rendering::post_processing as vc_ops;
use rusty_scene::Scene;
use rusty_shadergen::post_params::catalog_default;

pub(super) fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    register_names(lua, table, scene)?;
    register_params(lua, table, scene)
}

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
