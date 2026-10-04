//! `Graphics.{Get,Set}CustomEffects` (#397): the ordered authored post-FX modules the
//! active visual-correction volume runs, through the same
//! `authoring::visual_correction` op the Inspector's list calls — and
//! `Graphics.{Get,Set}PostParam` (#671), those modules' runtime params.

use std::cell::RefCell;

use mlua::{IntoLua, Value};

use super::super::material::shader_params::floats;
use super::super::{put, Reg};
use super::state::{with_vc, with_vc_mut};
use crate::scene::authoring::visual_correction as vc_ops;
use crate::scene::Scene;
use crate::shadergen::post_params::catalog_default;

/// `SetCustomEffects({names})` replaces the list (an invalid name is a Lua error and
/// nothing changes); `GetCustomEffects()` returns it, empty without an active volume.
pub(super) fn register_custom_effects<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetCustomEffects",
        scope.create_function(|_, names: Vec<String>| {
            let mut result = Ok(());
            with_vc_mut(scene, |e| result = vc_ops::set_custom_effects(e, names));
            result.map_err(mlua::Error::RuntimeError)
        }),
    )?;
    put(
        table,
        "GetCustomEffects",
        scope.create_function(|lua, ()| {
            let names = with_vc(scene, |vc| vc.custom_effects.clone()).unwrap_or_default();
            lua.create_sequence_from(names)
        }),
    )
}

/// `SetPostParam(name, value)` sets a runtime param of the active volume's effects
/// (strict: an unknown or baked name, a wrong arity, or no active volume is a Lua
/// error); `GetPostParam(name)` reads it back, or the catalog default with no volume.
pub(super) fn register_post_params<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetPostParam",
        scope.create_function(|_, (name, value): (String, Value)| {
            let value = floats(value)?;
            let mut result = Err("SetPostParam: no active post-processing volume".to_owned());
            with_vc_mut(scene, |e| result = vc_ops::set_post_param(e, &name, value));
            result.map_err(mlua::Error::RuntimeError)
        }),
    )?;
    put(
        table,
        "GetPostParam",
        scope.create_function(|lua, name: String| {
            let v = with_vc(scene, |vc| vc_ops::post_param(vc, &name))
                .unwrap_or_else(|| catalog_default(&name))
                .map_err(mlua::Error::RuntimeError)?;
            match v[..] {
                [x] => x.into_lua(lua),
                _ => v.into_lua(lua),
            }
        }),
    )
}
