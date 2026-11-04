//! `Graphics.{Get,Set}{Lift,Gamma,Gain}Wheel(r, g, b, offset)`: one pair per
//! wheel.

use super::super::super::super::{put, Reg};
use super::super::super::state::{with_vc, with_vc_mut};
use crate::api::ApiCell;
use crate::components::{ColorWheel, Wheel};
use crate::scene::authoring::rendering::post_processing::grading::lift_gamma_gain;
use crate::scene::Scene;

/// Every wheel's pair; neutral without an active volume, as a new volume starts.
pub(in crate::api::graphics) fn register(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
) -> Reg {
    for which in Wheel::ALL {
        register_wheel(lua, table, scene, which)?;
    }
    Ok(())
}

/// `Set<name>Wheel(r, g, b, offset)` / `Get<name>Wheel() -> r, g, b, offset`.
fn register_wheel(
    lua: &mlua::Lua,
    table: &mlua::Table,
    scene: ApiCell<Scene>,
    which: Wheel,
) -> Reg {
    let name = which.name();
    put(
        table,
        &format!("Set{name}Wheel"),
        lua.create_function(move |lua, (r, g, b, offset): (f32, f32, f32, f32)| {
            with_vc_mut(&*scene.get(lua)?, |e| {
                lift_gamma_gain::set_wheel(e, which, [r, g, b], offset)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        &format!("Get{name}Wheel"),
        lua.create_function(move |lua, ()| {
            let w = with_vc(&*scene.get(lua)?, |vc| *vc.lift_gamma_gain.wheel(which))
                .unwrap_or_default();
            let ColorWheel {
                color: [r, g, b],
                offset,
            } = w;
            Ok((r, g, b, offset))
        }),
    )
}
