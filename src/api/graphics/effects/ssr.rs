//! `Graphics.{Get,Set}Ssr{Active,Quality}`: screen-space reflections over the
//! active volume (gated by the High preset).

use std::cell::RefCell;

use super::super::super::{put, Reg};
use super::super::state::{with_vc, with_vc_mut};
use super::knobs::register_flag;
use crate::scene::authoring::rendering::post_processing::ssr;
use crate::scene::Scene;

/// The switch and the quality name (`false` / `""` without an active volume).
pub(super) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    register_flag(
        scope,
        (table, scene),
        "SsrActive",
        |vc| vc.ssr.active,
        ssr::set_active,
    )?;
    put(
        table,
        "SetSsrQuality",
        scope.create_function(|_, q: String| {
            with_vc_mut(scene, |e| ssr::set_quality(e, q.clone()));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetSsrQuality",
        scope.create_function(|_, ()| {
            Ok(with_vc(scene, |vc| vc.ssr.quality.clone()).unwrap_or_default())
        }),
    )
}
