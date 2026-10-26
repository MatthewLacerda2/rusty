//! `Graphics.{Get,Set}Tonemap(name)` and the display-gamma knob
//! `Graphics.{Get,Set}Gamma`.

use std::cell::RefCell;

use super::super::super::super::{put, Reg};
use super::super::super::state::{with_vc, with_vc_mut};
use crate::components::Tonemap;
use crate::scene::authoring::rendering::post_processing::grading::tonemapping;
use crate::scene::Scene;

/// The operator, by Unity's name (an unknown one is ignored; ACES without an
/// active volume), and the knob.
pub(in crate::api::graphics) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetTonemap",
        scope.create_function(|_, name: String| {
            if let Some(tm) = Tonemap::parse(&name) {
                with_vc_mut(scene, |e| tonemapping::set_mode(e, tm));
            }
            Ok(())
        }),
    )?;
    put(
        table,
        "GetTonemap",
        scope.create_function(|_, ()| {
            Ok(with_vc(scene, |vc| vc.tonemapping.mode)
                .unwrap_or_default()
                .name()
                .to_string())
        }),
    )?;
    super::super::knobs::register(scope, table, scene, tonemapping::EFFECT.knobs)
}
