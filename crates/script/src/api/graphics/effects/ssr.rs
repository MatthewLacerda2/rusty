//! `Graphics.{Get,Set}Ssr{Active,Quality}` and the `Custom` march's knobs,
//! `Graphics.{Get,Set}Ssr{Steps,StepLength}` (#1522), over the active volume.

use super::super::super::Reg;
use super::knobs::{register_count, register_flag, register_quality};
use crate::api::ApiCell;
use rusty_scene::authoring::rendering::post_processing::ssr;
use rusty_scene::Scene;

/// The switch, the quality level and the knobs (a new volume's values without an
/// active volume, `false` for the switch).
pub(super) fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    register_flag(
        lua,
        (table, scene),
        "SsrActive",
        |vc| vc.ssr.active,
        ssr::set_active,
    )?;
    register_quality(
        lua,
        (table, scene),
        "SsrQuality",
        |vc| vc.ssr.quality,
        ssr::set_quality,
    )?;
    register_count(
        lua,
        (table, scene),
        "SsrSteps",
        |vc| vc.ssr.steps,
        ssr::set_steps,
    )?;
    super::knobs::register(lua, table, scene, ssr::EFFECT.knobs)
}
