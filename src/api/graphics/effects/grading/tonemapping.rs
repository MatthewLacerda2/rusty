//! `Graphics.{Get,Set}Tonemap(name)` and the display-gamma knob
//! `Graphics.{Get,Set}Gamma`.

use super::super::super::super::{put, Reg};
use super::super::super::state::{with_vc, with_vc_mut};
use crate::api::ApiCell;
use crate::components::Tonemap;
use crate::scene::authoring::rendering::post_processing::grading::tonemapping;
use crate::scene::Scene;

/// The operator, by Unity's name (an unknown one is ignored; ACES without an
/// active volume), and the knob.
pub(in crate::api::graphics) fn register(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
) -> Reg {
    put(
        table,
        "SetTonemap",
        lua.create_function(move |lua, name: String| {
            if let Some(tm) = Tonemap::parse(&name) {
                with_vc_mut(&*scene.get(lua)?, |e| tonemapping::set_mode(e, tm));
            }
            Ok(())
        }),
    )?;
    put(
        table,
        "GetTonemap",
        lua.create_function(move |lua, ()| {
            Ok(with_vc(&*scene.get(lua)?, |vc| vc.tonemapping.mode)
                .unwrap_or_default()
                .name()
                .to_string())
        }),
    )?;
    super::super::knobs::register(lua, table, scene, tonemapping::EFFECT.knobs)
}
