//! `Graphics.{Get,Set}Ssao{Active,Radius,Intensity}` (#436) and its quality,
//! `Graphics.{Get,Set}Ssao{Quality,Samples,ResolutionDivisor}` (#1522): screen-space
//! ambient occlusion on the active post-processing volume, through the same
//! `authoring::rendering::post_processing` ops the Inspector card calls.

use std::cell::RefCell;

use super::super::{put, Reg};
use super::effects::knobs::{register_count, register_quality};
use super::state::{with_vc, with_vc_mut};
use crate::components::SsaoSettings;
use crate::scene::authoring::rendering::post_processing as vc_ops;
use crate::scene::Scene;

/// Active flag, radius and intensity. AO lives on the volume, so without an active
/// one `GetSsaoActive` reports `false` (nothing runs) and the other getters report
/// the defaults a new volume would start with.
pub(super) fn register_ssao<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let defaults = SsaoSettings::default();
    put(
        table,
        "SetSsaoActive",
        scope.create_function(|_, on: bool| {
            with_vc_mut(scene, |e| vc_ops::volume::set_ssao_active(e, on));
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
            with_vc_mut(scene, |e| vc_ops::volume::set_ssao_radius(e, radius));
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
            with_vc_mut(scene, |e| vc_ops::volume::set_ssao_intensity(e, intensity));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetSsaoIntensity",
        scope.create_function(move |_, ()| {
            Ok(with_vc(scene, |vc| vc.ssao.intensity).unwrap_or(defaults.intensity))
        }),
    )?;
    register_ssao_quality(scope, table, scene)
}

/// The quality level and its `Custom` sample count and resolution divisor (#1522).
fn register_ssao_quality<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    register_quality(
        scope,
        (table, scene),
        "SsaoQuality",
        |vc| vc.ssao.quality,
        vc_ops::volume::set_ssao_quality,
    )?;
    register_count(
        scope,
        (table, scene),
        "SsaoSamples",
        |vc| vc.ssao.samples,
        vc_ops::volume::set_ssao_samples,
    )?;
    register_count(
        scope,
        (table, scene),
        "SsaoResolutionDivisor",
        |vc| vc.ssao.resolution_divisor,
        vc_ops::volume::set_ssao_resolution_divisor,
    )
}
