use super::super::super::Reg;
use super::knobs::{register_count, register_flag, register_quality};
use crate::api::ApiCell;
use rusty_edit::authoring::rendering::post_processing::ssil;
use rusty_scene::Scene;

pub(super) fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    let cx = (table, scene);
    register_flag(lua, cx, "SsilActive", |vc| vc.ssil.active, ssil::set_active)?;
    register_quality(
        lua,
        cx,
        "SsilQuality",
        |vc| vc.ssil.quality,
        ssil::set_quality,
    )?;
    register_count(
        lua,
        cx,
        "SsilSamples",
        |vc| vc.ssil.samples,
        ssil::set_samples,
    )?;
    register_count(
        lua,
        cx,
        "SsilResolutionDivisor",
        |vc| vc.ssil.resolution_divisor,
        ssil::set_resolution_divisor,
    )?;
    super::knobs::register(lua, table, scene, ssil::EFFECT.knobs)
}
