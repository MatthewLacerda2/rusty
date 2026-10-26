//! `Graphics.{Get,Set}{Lift,Gamma,Gain}Wheel(r, g, b, offset)`: one pair per
//! wheel.

use std::cell::RefCell;

use super::super::super::super::{put, Reg};
use super::super::super::state::{with_vc, with_vc_mut};
use crate::components::{ColorWheel, Wheel};
use crate::scene::authoring::rendering::post_processing::grading::lift_gamma_gain;
use crate::scene::Scene;

/// Every wheel's pair; neutral without an active volume, as a new volume starts.
pub(in crate::api::graphics) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    for which in Wheel::ALL {
        register_wheel(scope, table, scene, which)?;
    }
    Ok(())
}

/// `Set<name>Wheel(r, g, b, offset)` / `Get<name>Wheel() -> r, g, b, offset`.
fn register_wheel<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
    which: Wheel,
) -> Reg {
    let name = which.name();
    put(
        table,
        &format!("Set{name}Wheel"),
        scope.create_function(move |_, (r, g, b, offset): (f32, f32, f32, f32)| {
            with_vc_mut(scene, |e| {
                lift_gamma_gain::set_wheel(e, which, [r, g, b], offset)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        &format!("Get{name}Wheel"),
        scope.create_function(move |_, ()| {
            let w = with_vc(scene, |vc| *vc.lift_gamma_gain.wheel(which)).unwrap_or_default();
            let ColorWheel {
                color: [r, g, b],
                offset,
            } = w;
            Ok((r, g, b, offset))
        }),
    )
}
