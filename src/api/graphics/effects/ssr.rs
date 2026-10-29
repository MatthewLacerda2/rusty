//! `Graphics.{Get,Set}Ssr{Active,Quality}` and the `Custom` march's knobs,
//! `Graphics.{Get,Set}Ssr{Steps,StepLength}` (#1522), over the active volume.

use std::cell::RefCell;

use super::super::super::Reg;
use super::knobs::{register_count, register_flag, register_quality};
use crate::scene::authoring::rendering::post_processing::ssr;
use crate::scene::Scene;

/// The switch, the quality level and the knobs (a new volume's values without an
/// active volume, `false` for the switch).
pub(super) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    register_flag(
        scope,
        (table, scene),
        "SsrActive",
        |vc| vc.ssr.active,
        ssr::set_active,
    )?;
    register_quality(
        scope,
        (table, scene),
        "SsrQuality",
        |vc| vc.ssr.quality,
        ssr::set_quality,
    )?;
    register_count(
        scope,
        (table, scene),
        "SsrSteps",
        |vc| vc.ssr.steps,
        ssr::set_steps,
    )?;
    super::knobs::register(scope, table, scene, ssr::EFFECT.knobs)
}
