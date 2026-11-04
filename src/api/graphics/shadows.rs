//! `Graphics.{Get,Set}Shadow{Cascades,Distance}` (#435): the sun's cascaded shadow
//! map over the active post-processing volume, through the same
//! `authoring::rendering::post_processing` ops the Inspector card calls.

use super::super::{put, Reg};
use super::state::{with_vc, with_vc_mut};
use crate::api::ApiCell;
use crate::components::ShadowSettings;
use crate::scene::authoring::rendering::post_processing as vc_ops;
use crate::scene::Scene;

/// Cascade count and shadow distance. The getters report the engine defaults when
/// no volume is active, since that is what the renderer then uses.
pub(super) fn register_shadows(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    let defaults = ShadowSettings::default();
    put(
        table,
        "SetShadowCascades",
        lua.create_function(move |lua, count: u32| {
            with_vc_mut(&*scene.get(lua)?, |e| {
                vc_ops::volume::set_shadow_cascades(e, count)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "GetShadowCascades",
        lua.create_function(move |lua, ()| {
            Ok(with_vc(&*scene.get(lua)?, |vc| vc.shadows.cascades).unwrap_or(defaults.cascades))
        }),
    )?;
    put(
        table,
        "SetShadowDistance",
        lua.create_function(move |lua, distance: f32| {
            with_vc_mut(&*scene.get(lua)?, |e| {
                vc_ops::volume::set_shadow_distance(e, distance)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "GetShadowDistance",
        lua.create_function(move |lua, ()| {
            Ok(with_vc(&*scene.get(lua)?, |vc| vc.shadows.distance).unwrap_or(defaults.distance))
        }),
    )
}
