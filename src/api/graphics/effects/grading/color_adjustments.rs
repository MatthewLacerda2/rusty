//! `Graphics.{Get,Set}{Contrast,Saturation,HueShift}`: the color adjustments,
//! every one a knob.

use super::super::super::super::Reg;
use crate::api::ApiCell;
use crate::scene::authoring::rendering::post_processing::grading::color_adjustments;
use crate::scene::Scene;

/// Its knobs; neutral without an active volume, as a new volume starts.
pub(in crate::api::graphics) fn register(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
) -> Reg {
    super::super::knobs::register(lua, table, scene, color_adjustments::EFFECT.knobs)
}
