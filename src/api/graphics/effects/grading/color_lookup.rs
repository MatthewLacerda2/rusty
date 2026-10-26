//! `Graphics.{Get,Set}ColorLookup(path | nil)` (#1441) and its contribution knob
//! `Graphics.{Get,Set}ColorLookupContribution`.

use std::cell::RefCell;

use super::super::super::super::{put, Reg};
use super::super::super::state::{with_vc, with_vc_mut};
use crate::scene::authoring::rendering::post_processing::grading::color_lookup;
use crate::scene::Scene;

/// The LUT path (`nil` without one, or without an active volume) and the knob.
pub(in crate::api::graphics) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetColorLookup",
        scope.create_function(|_, path: Option<String>| {
            with_vc_mut(scene, |e| color_lookup::set_lut(e, path));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetColorLookup",
        scope.create_function(|_, ()| {
            Ok(with_vc(scene, |vc| vc.color_lookup.lut.clone()).flatten())
        }),
    )?;
    super::super::knobs::register(scope, table, scene, color_lookup::EFFECT.knobs)
}
