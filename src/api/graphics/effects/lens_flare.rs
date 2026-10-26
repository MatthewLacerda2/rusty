//! `Graphics.{Get,Set}LensFlare{Active,Intensity,GhostCount,GhostIntensity,
//! HaloIntensity,StreakIntensity,Tint,ChromaticAberration}` (#1443).

use std::cell::RefCell;

use super::super::super::{put, Reg};
use super::super::state::{with_vc, with_vc_mut};
use super::knobs::{register as knobs, register_flag};
use crate::components::rendering::post_processing::LensFlare;
use crate::scene::authoring::rendering::post_processing::lens_flare;
use crate::scene::Scene;

/// The switch, the knobs, the ghost count and the tint (a new volume's without an
/// active volume).
pub(super) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let set = lens_flare::set_active;
    register_flag(
        scope,
        (table, scene),
        "LensFlareActive",
        |vc| vc.lens_flare.active,
        set,
    )?;
    knobs(scope, table, scene, lens_flare::EFFECT.knobs)?;
    register_count_and_tint(scope, table, scene)
}

/// `SetLensFlareGhostCount(n)` / `GetLensFlareGhostCount() -> n` and
/// `SetLensFlareTint(r, g, b)` / `GetLensFlareTint() -> r, g, b`.
fn register_count_and_tint<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let defaults = LensFlare::DEFAULT;
    put(
        table,
        "SetLensFlareGhostCount",
        scope.create_function(|_, n: u32| {
            with_vc_mut(scene, |e| lens_flare::set_ghost_count(e, n));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetLensFlareGhostCount",
        scope.create_function(move |_, ()| {
            let n = with_vc(scene, |vc| vc.lens_flare.ghost_count);
            Ok(n.unwrap_or(defaults.ghost_count))
        }),
    )?;
    put(
        table,
        "SetLensFlareTint",
        scope.create_function(|_, (r, g, b): (f32, f32, f32)| {
            with_vc_mut(scene, |e| lens_flare::set_tint(e, [r, g, b]));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetLensFlareTint",
        scope.create_function(move |_, ()| {
            let [r, g, b] = with_vc(scene, |vc| vc.lens_flare.tint).unwrap_or(defaults.tint);
            Ok((r, g, b))
        }),
    )
}
