//! `Graphics.{Get,Set}CustomEffects` (#397): the ordered authored post-FX modules the
//! active visual-correction volume runs, through the same
//! `authoring::visual_correction` op the Inspector's list calls.

use std::cell::RefCell;

use super::super::{put, Reg};
use super::state::{with_vc, with_vc_mut};
use crate::scene::authoring::visual_correction as vc_ops;
use crate::scene::Scene;

/// `SetCustomEffects({names})` replaces the list (an invalid name is a Lua error and
/// nothing changes); `GetCustomEffects()` returns it, empty without an active volume.
pub(super) fn register_custom_effects<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
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
