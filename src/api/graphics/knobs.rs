//! The `Graphics.Get/Set<api>` pair of every effect knob that is one on its own
//! (#1467): each effect's knob table (`authoring::rendering::post_processing`)
//! names the pair, so the functions, the Inspector's sliders and the
//! `Graphics.<effect>.<knob>` tween paths reach the same value through one op.

use std::cell::RefCell;

use super::super::{put, Reg};
use super::state::{with_vc, with_vc_mut};
use crate::scene::authoring::default_post_processing;
use crate::scene::authoring::rendering::post_processing::Knob;
use crate::scene::Scene;

/// Register `Set<api>(value)` / `Get<api>()` for each of `knobs` that names an
/// `api`. Without an active volume the setter does nothing and the getter reports
/// what a new volume starts with.
pub(super) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    knobs: &'static [Knob],
) -> Reg {
    for knob in knobs {
        let Some(api) = knob.api else { continue };
        put(
            table,
            &format!("Set{api}"),
            scope.create_function(move |_, value: f32| {
                with_vc_mut(scene, |vc| (knob.set)(vc, value));
                Ok(())
            }),
        )?;
        put(
            table,
            &format!("Get{api}"),
            scope.create_function(move |_, ()| {
                let value = with_vc(scene, knob.get);
                Ok(value.unwrap_or_else(|| (knob.get)(&default_post_processing())))
            }),
        )?;
    }
    Ok(())
}
