//! `Graphics.{Get,Set}Ssao{Active,Radius,Intensity}` (#436): screen-space ambient
//! occlusion on the active visual-correction volume, through the same
//! `authoring::visual_correction` ops the Inspector card calls.

use std::cell::RefCell;

use super::super::{put, Reg};
use super::state::{with_vc, with_vc_mut};
use crate::components::SsaoSettings;
use crate::scene::authoring::visual_correction as vc_ops;
use crate::scene::Scene;

/// Active flag, radius and intensity. AO lives on the volume, so without an active
/// one `GetSsaoActive` reports `false` (nothing runs) and the other getters report
/// the defaults a new volume would start with.
pub(super) fn register_ssao<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let defaults = SsaoSettings::default();
    put(
        table,
        "SetSsaoActive",
        scope.create_function(|_, on: bool| {
            with_vc_mut(scene, |e| vc_ops::set_ssao_active(e, on));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetSsaoActive",
        scope.create_function(|_, ()| Ok(with_vc(scene, |vc| vc.ssao.active).unwrap_or(false))),
    )?;
    put(
        table,
        "SetSsaoRadius",
        scope.create_function(|_, radius: f32| {
            with_vc_mut(scene, |e| vc_ops::set_ssao_radius(e, radius));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetSsaoRadius",
        scope.create_function(move |_, ()| {
            Ok(with_vc(scene, |vc| vc.ssao.radius).unwrap_or(defaults.radius))
        }),
    )?;
    put(
        table,
        "SetSsaoIntensity",
        scope.create_function(|_, intensity: f32| {
            with_vc_mut(scene, |e| vc_ops::set_ssao_intensity(e, intensity));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetSsaoIntensity",
        scope.create_function(move |_, ()| {
            Ok(with_vc(scene, |vc| vc.ssao.intensity).unwrap_or(defaults.intensity))
        }),
    )
}
