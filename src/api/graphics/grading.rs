//! `Graphics.{Get,Set}WhiteBalance`, `{Get,Set}{Lift,Gamma,Gain}Wheel` and
//! `{Get,Set}HueShift` (#1440), and `{Get,Set}ColorLookup` /
//! `{Get,Set}ColorLookupContribution` (#1441): the colour grade on the active
//! post-processing volume, through the same `authoring::rendering::post_processing` ops the
//! Inspector card calls. The renderer bakes them into the view's grading LUT.

use std::cell::RefCell;

use super::super::{put, Reg};
use super::state::{with_vc, with_vc_mut};
use crate::components::{ColorWheel, Wheel};
use crate::scene::authoring::rendering::post_processing as vc_ops;
use crate::scene::Scene;

/// White balance, the three wheels and the hue shift. Without an active volume the
/// getters report the neutral values a new volume starts with.
pub(super) fn register_grading<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetWhiteBalance",
        scope.create_function(|_, (temperature, tint): (f32, f32)| {
            with_vc_mut(scene, |e| {
                vc_ops::grading::white_balance::set(e, temperature, tint)
            });
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
    )?;
    for which in Wheel::ALL {
        register_wheel(scope, table, scene, which)?;
    }
    put(
        table,
        "SetHueShift",
        scope.create_function(|_, degrees: f32| {
            with_vc_mut(scene, |e| {
                vc_ops::grading::color_adjustments::set_hue_shift(e, degrees)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "GetHueShift",
        scope.create_function(|_, ()| {
            Ok(with_vc(scene, |vc| vc.color_adjustments.hue_shift).unwrap_or(0.0))
        }),
    )?;
    register_color_lookup(scope, table, scene)
}

/// `SetColorLookup(path | nil)` / `GetColorLookup() -> path | nil`, and the
/// contribution (`1` without an active volume, as a new one starts).
fn register_color_lookup<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetColorLookup",
        scope.create_function(|_, path: Option<String>| {
            with_vc_mut(scene, |e| vc_ops::grading::color_lookup::set_lut(e, path));
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
    put(
        table,
        "SetColorLookupContribution",
        scope.create_function(|_, value: f32| {
            with_vc_mut(scene, |e| {
                vc_ops::grading::color_lookup::set_contribution(e, value)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "GetColorLookupContribution",
        scope.create_function(|_, ()| {
            Ok(with_vc(scene, |vc| vc.color_lookup.contribution).unwrap_or(1.0))
        }),
    )
}

/// `Set<name>Wheel(r, g, b, offset)` / `Get<name>Wheel() -> r, g, b, offset`.
fn register_wheel<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    which: Wheel,
) -> Reg {
    let name = which.name();
    put(
        table,
        &format!("Set{name}Wheel"),
        scope.create_function(move |_, (r, g, b, offset): (f32, f32, f32, f32)| {
            with_vc_mut(scene, |e| {
                vc_ops::grading::lift_gamma_gain::set_wheel(e, which, [r, g, b], offset)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        &format!("Get{name}Wheel"),
        scope.create_function(move |_, ()| {
            let w = with_vc(scene, |vc| *vc.lift_gamma_gain.wheel(which)).unwrap_or_default();
            let ColorWheel {
                color: [r, g, b],
                offset,
            } = w;
            Ok((r, g, b, offset))
        }),
    )
}
