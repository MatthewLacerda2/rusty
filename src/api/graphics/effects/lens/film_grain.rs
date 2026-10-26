//! `Graphics.{Get,Set}FilmGrain{Active,Intensity,Response}` (#1439).

use std::cell::RefCell;

use super::super::super::super::Reg;
use super::super::knobs::{register as knobs, register_flag};
use crate::scene::authoring::rendering::post_processing::lens::film_grain;
use crate::scene::Scene;

/// The switch and the knobs.
pub(in crate::api::graphics) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let set = film_grain::set_active;
    register_flag(
        scope,
        (table, scene),
        "FilmGrainActive",
        |vc| vc.film_grain.active,
        set,
    )?;
    knobs(scope, table, scene, film_grain::EFFECT.knobs)
}
