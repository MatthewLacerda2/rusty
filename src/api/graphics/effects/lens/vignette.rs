//! `Graphics.{Get,Set}Vignette{Active,Intensity,Smoothness,Color,Center}` (#1439).

use std::cell::RefCell;

use super::super::super::super::{put, Reg};
use super::super::super::state::{with_vc, with_vc_mut};
use super::super::knobs::{register as knobs, register_flag};
use crate::components::Vignette;
use crate::scene::authoring::rendering::post_processing::lens::vignette;
use crate::scene::Scene;

/// The switch, the knobs, and the colour and centre (a new volume's without an
/// active volume).
pub(in crate::api::graphics) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let set = vignette::set_active;
    register_flag(
        scope,
        (table, scene),
        "VignetteActive",
        |vc| vc.vignette.active,
        set,
    )?;
    knobs(scope, table, scene, vignette::EFFECT.knobs)?;
    register_vectors(scope, table, scene)
}

/// `SetVignetteColor(r, g, b)` / `GetVignetteColor() -> r, g, b` and
/// `SetVignetteCenter(x, y)` / `GetVignetteCenter() -> x, y`.
fn register_vectors<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let defaults = Vignette::DEFAULT;
    put(
        table,
        "SetVignetteColor",
        scope.create_function(|_, (r, g, b): (f32, f32, f32)| {
            with_vc_mut(scene, |e| vignette::set_color(e, [r, g, b]));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetVignetteColor",
        scope.create_function(move |_, ()| {
            let [r, g, b] = with_vc(scene, |vc| vc.vignette.color).unwrap_or(defaults.color);
            Ok((r, g, b))
        }),
    )?;
    put(
        table,
        "SetVignetteCenter",
        scope.create_function(|_, (x, y): (f32, f32)| {
            with_vc_mut(scene, |e| vignette::set_center(e, [x, y]));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetVignetteCenter",
        scope.create_function(move |_, ()| {
            let [x, y] = with_vc(scene, |vc| vc.vignette.center).unwrap_or(defaults.center);
            Ok((x, y))
        }),
    )
}
