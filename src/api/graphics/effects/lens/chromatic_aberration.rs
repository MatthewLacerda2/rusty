//! `Graphics.{Get,Set}ChromaticAberration{Active,Intensity}` (#1439).

use std::cell::RefCell;

use super::super::super::super::Reg;
use super::super::knobs::{register as knobs, register_flag};
use crate::components::PostProcessingComponent;
use crate::scene::authoring::rendering::post_processing::lens::chromatic_aberration as ca;
use crate::scene::Scene;

/// The switch and the knob.
pub(in crate::api::graphics) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let get = |vc: &PostProcessingComponent| vc.chromatic_aberration.active;
    register_flag(
        scope,
        (table, scene),
        "ChromaticAberrationActive",
        get,
        ca::set_active,
    )?;
    knobs(scope, table, scene, ca::EFFECT.knobs)
}
