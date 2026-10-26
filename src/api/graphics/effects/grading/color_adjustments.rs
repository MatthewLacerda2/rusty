//! `Graphics.{Get,Set}{Contrast,Saturation,HueShift}`: the color adjustments,
//! every one a knob.

use std::cell::RefCell;

use super::super::super::super::Reg;
use crate::scene::authoring::rendering::post_processing::grading::color_adjustments;
use crate::scene::Scene;

/// Its knobs; neutral without an active volume, as a new volume starts.
pub(in crate::api::graphics) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    super::super::knobs::register(scope, table, scene, color_adjustments::EFFECT.knobs)
}
