//! `Graphics.{Get,Set}ChromaticAberration{Active,Intensity}` (#1439).

use super::super::super::super::Reg;
use super::super::knobs::{register as knobs, register_flag};
use crate::api::ApiCell;
use crate::components::PostProcessingComponent;
use crate::scene::authoring::rendering::post_processing::lens::chromatic_aberration as ca;
use crate::scene::Scene;

/// The switch and the knob.
pub(in crate::api::graphics) fn register(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
) -> Reg {
    let get = |vc: &PostProcessingComponent| vc.chromatic_aberration.active;
    register_flag(
        lua,
        (table, scene),
        "ChromaticAberrationActive",
        get,
        ca::set_active,
    )?;
    knobs(lua, table, scene, ca::EFFECT.knobs)
}
