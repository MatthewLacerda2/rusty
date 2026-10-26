//! `Graphics.{Get,Set}Bloom{Active,Intensity,Threshold}` over the active volume.

use std::cell::RefCell;

use super::super::super::{put, Reg};
use super::super::state::{with_vc, with_vc_mut};
use crate::scene::authoring::rendering::post_processing::bloom;
use crate::scene::Scene;

/// The switch (`false` without an active volume) and the two knobs.
pub(super) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetBloomActive",
        scope.create_function(|_, active: bool| {
            with_vc_mut(scene, |e| bloom::set_active(e, active));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetBloomActive",
        scope.create_function(|_, ()| Ok(with_vc(scene, |vc| vc.bloom.active).unwrap_or(false))),
    )?;
    super::knobs::register(scope, table, scene, bloom::EFFECT.knobs)
}
