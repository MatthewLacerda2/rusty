//! `Graphics.{Get,Set}Dummy{Active,Strength}`: the test-only dummy effect (#1467).

use std::cell::RefCell;

use super::super::super::Reg;
use super::knobs::{register as knobs, register_flag};
use crate::scene::authoring::rendering::post_processing::dummy;
use crate::scene::Scene;

/// The switch and the knob.
pub(super) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    let set = dummy::set_active;
    register_flag(
        scope,
        (table, scene),
        "DummyActive",
        |vc| vc.dummy.active,
        set,
    )?;
    knobs(scope, table, scene, dummy::EFFECT.knobs)
}
