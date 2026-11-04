//! `Graphics.{Get,Set}Dummy{Active,Strength}`: the test-only dummy effect (#1467).

use super::super::super::Reg;
use super::knobs::{register as knobs, register_flag};
use crate::api::ApiCell;
use crate::scene::authoring::rendering::post_processing::dummy;
use crate::scene::Scene;

/// The switch and the knob.
pub(super) fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    let set = dummy::set_active;
    register_flag(
        lua,
        (table, scene),
        "DummyActive",
        |vc| vc.dummy.active,
        set,
    )?;
    knobs(lua, table, scene, dummy::EFFECT.knobs)
}
