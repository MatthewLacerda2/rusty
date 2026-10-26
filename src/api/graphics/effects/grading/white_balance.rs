//! `Graphics.{Get,Set}WhiteBalance(temperature, tint)`: both knobs in one pair.

use std::cell::RefCell;

use super::super::super::super::{put, Reg};
use super::super::super::state::{with_vc, with_vc_mut};
use crate::scene::authoring::rendering::post_processing::grading::white_balance;
use crate::scene::Scene;

/// The pair; neutral without an active volume, as a new volume starts.
pub(in crate::api::graphics) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetWhiteBalance",
        scope.create_function(|_, (temperature, tint): (f32, f32)| {
            with_vc_mut(scene, |e| white_balance::set(e, temperature, tint));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetWhiteBalance",
        scope.create_function(|_, ()| {
            let wb = with_vc(scene, |vc| vc.white_balance).unwrap_or_default();
            Ok((wb.temperature, wb.tint))
        }),
    )
}
