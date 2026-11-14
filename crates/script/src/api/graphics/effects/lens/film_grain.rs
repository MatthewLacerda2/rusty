//! `Graphics.{Get,Set}FilmGrain{Active,Intensity,Response}` (#1439).

use super::super::super::super::Reg;
use super::super::knobs::{register as knobs, register_flag};
use crate::api::ApiCell;
use rusty_scene::authoring::rendering::post_processing::lens::film_grain;
use rusty_scene::Scene;

/// The switch and the knobs.
pub(in crate::api::graphics) fn register(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
) -> Reg {
    let set = film_grain::set_active;
    register_flag(
        lua,
        (table, scene),
        "FilmGrainActive",
        |vc| vc.film_grain.active,
        set,
    )?;
    knobs(lua, table, scene, film_grain::EFFECT.knobs)
}
