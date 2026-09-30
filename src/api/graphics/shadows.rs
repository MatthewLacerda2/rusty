//! `Graphics.{Get,Set}Shadow{Cascades,Distance}` (#435): the sun's cascaded shadow
//! map over the active visual-correction volume, through the same
//! `authoring::visual_correction` ops the Inspector card calls.

use std::cell::RefCell;

use super::super::{put, Reg};
use super::state::{with_vc, with_vc_mut};
use crate::components::ShadowSettings;
use crate::scene::authoring::visual_correction as vc_ops;
use crate::scene::Scene;

/// Cascade count and shadow distance. The getters report the engine defaults when
/// no volume is active, since that is what the renderer then uses.
pub(super) fn register_shadows<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let defaults = ShadowSettings::default();
    put(
        table,
        "SetShadowCascades",
        scope.create_function(|_, count: u32| {
            with_vc_mut(scene, |e| vc_ops::set_shadow_cascades(e, count));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetShadowCascades",
        scope.create_function(move |_, ()| {
            Ok(with_vc(scene, |vc| vc.shadows.cascades).unwrap_or(defaults.cascades))
        }),
    )?;
    put(
        table,
        "SetShadowDistance",
        scope.create_function(|_, distance: f32| {
            with_vc_mut(scene, |e| vc_ops::set_shadow_distance(e, distance));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetShadowDistance",
        scope.create_function(move |_, ()| {
            Ok(with_vc(scene, |vc| vc.shadows.distance).unwrap_or(defaults.distance))
        }),
    )
}
