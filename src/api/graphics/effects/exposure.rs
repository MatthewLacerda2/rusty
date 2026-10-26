//! `Graphics.{Get,Set}Exposure`, `{Get,Set}ExposureMode` and the automatic mode's
//! knobs (#1444): limits, compensation, adaptation speeds and histogram window, on
//! the active post-processing volume, through the same
//! `authoring::rendering::post_processing` ops the Inspector card calls.

use std::cell::RefCell;

use super::super::super::{put, Reg};
use super::super::state::{with_vc, with_vc_mut};
use crate::components::ExposureMode;
use crate::scene::authoring::rendering::post_processing as vc_ops;
use crate::scene::Scene;

/// The mode, the automatic mode's paired knobs, and the knobs on their own (the
/// fixed EV and the compensation). Without an active volume the getters report what
/// a new volume starts with. `GetExposure` is the configured EV in both modes, never
/// the adapted one: scripts run in the sim, and the adapted EV is a GPU readback
/// (the dev-only `Debug.GetAdaptedExposure` reads that).
pub(super) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetExposureMode",
        scope.create_function(|_, name: String| {
            if let Some(mode) = ExposureMode::parse(&name) {
                with_vc_mut(scene, |e| vc_ops::exposure::set_mode(e, mode));
            }
            Ok(())
        }),
    )?;
    put(
        table,
        "GetExposureMode",
        scope.create_function(|_, ()| {
            let mode = with_vc(scene, |vc| vc.exposure.mode).unwrap_or_default();
            Ok(mode.name().to_string())
        }),
    )?;
    register_limits(scope, table, scene)?;
    register_adaptation(scope, table, scene)?;
    super::knobs::register(scope, table, scene, vc_ops::exposure::EFFECT.knobs)
}

/// The automatic mode's limits.
fn register_limits<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let auto = move || with_vc(scene, |vc| vc.exposure.automatic).unwrap_or_default();
    put(
        table,
        "SetExposureLimits",
        scope.create_function(|_, (min, max): (f32, f32)| {
            with_vc_mut(scene, |e| vc_ops::exposure::set_limits(e, min, max));
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
    )
}

/// The automatic mode's adaptation speeds and histogram window.
fn register_adaptation<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let auto = move || with_vc(scene, |vc| vc.exposure.automatic).unwrap_or_default();
    put(
        table,
        "SetAdaptationSpeed",
        scope.create_function(|_, (up, down): (f32, f32)| {
            with_vc_mut(scene, |e| {
                vc_ops::exposure::set_adaptation_speed(e, up, down)
            });
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
            with_vc_mut(scene, |e| {
                vc_ops::exposure::set_histogram_percent(e, low, high)
            });
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
