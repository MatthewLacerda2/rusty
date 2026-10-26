//! `Graphics.{Get,Set}Exposure`, `{Get,Set}ExposureMode` and the automatic mode's
//! knobs (#1444): limits, compensation, adaptation speeds and histogram window, on
//! the active post-processing volume, through the same
//! `authoring::rendering::post_processing` ops the Inspector card calls.

use std::cell::RefCell;

use super::super::{put, Reg};
use super::state::{with_vc, with_vc_mut};
use crate::components::{ExposureMode, PostProcessingComponent};
use crate::scene::authoring::rendering::post_processing as vc_ops;
use crate::scene::Scene;

/// The exposure and its mode. Without an active volume the getters report what a
/// new volume starts with.
pub(super) fn register_exposure<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetExposure",
        scope.create_function(|_, v: f32| {
            with_vc_mut(scene, |e| vc_ops::set_exposure(e, v));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetExposure",
        scope.create_function(|_, ()| {
            let readout = scene.borrow().screen_exposure;
            Ok(with_vc(scene, |vc| applied_exposure(vc, readout)).unwrap_or(0.0))
        }),
    )?;
    put(
        table,
        "SetExposureMode",
        scope.create_function(|_, name: String| {
            if let Some(mode) = ExposureMode::parse(&name) {
                with_vc_mut(scene, |e| vc_ops::set_exposure_mode(e, mode));
            }
            Ok(())
        }),
    )?;
    put(
        table,
        "GetExposureMode",
        scope.create_function(|_, ()| {
            let mode = with_vc(scene, |vc| vc.exposure_mode).unwrap_or_default();
            Ok(mode.name().to_string())
        }),
    )?;
    register_auto_exposure(scope, table, scene)
}

/// The exposure `vc` applies: its fixed EV, or in `Automatic` the screen's adapted
/// EV once a frame has metered it (`readout`, `Scene::screen_exposure`).
fn applied_exposure(vc: &PostProcessingComponent, readout: Option<f32>) -> f32 {
    match vc.exposure_mode {
        ExposureMode::Fixed => vc.exposure,
        ExposureMode::Automatic => readout.unwrap_or(vc.exposure),
    }
}

/// The automatic mode's knobs, each pair set and read together.
fn register_auto_exposure<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let auto = move || with_vc(scene, |vc| vc.auto_exposure).unwrap_or_default();
    put(
        table,
        "SetExposureLimits",
        scope.create_function(|_, (min, max): (f32, f32)| {
            with_vc_mut(scene, |e| vc_ops::set_exposure_limits(e, min, max));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetExposureLimits",
        scope.create_function(move |_, ()| {
            let a = auto();
            Ok((a.min_ev, a.max_ev))
        }),
    )?;
    put(
        table,
        "SetExposureCompensation",
        scope.create_function(|_, ev: f32| {
            with_vc_mut(scene, |e| vc_ops::set_exposure_compensation(e, ev));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetExposureCompensation",
        scope.create_function(move |_, ()| Ok(auto().compensation)),
    )?;
    put(
        table,
        "SetAdaptationSpeed",
        scope.create_function(|_, (up, down): (f32, f32)| {
            with_vc_mut(scene, |e| vc_ops::set_adaptation_speed(e, up, down));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetAdaptationSpeed",
        scope.create_function(move |_, ()| {
            let a = auto();
            Ok((a.speed_up, a.speed_down))
        }),
    )?;
    put(
        table,
        "SetHistogramPercent",
        scope.create_function(|_, (low, high): (f32, f32)| {
            with_vc_mut(scene, |e| vc_ops::set_histogram_percent(e, low, high));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetHistogramPercent",
        scope.create_function(move |_, ()| {
            let a = auto();
            Ok((a.low_percent, a.high_percent))
        }),
    )
}
