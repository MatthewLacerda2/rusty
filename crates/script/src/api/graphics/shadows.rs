use super::super::{put, Reg};
use super::state::{with_vc, with_vc_mut};
use crate::api::ApiCell;
use rusty_components::ShadowSettings;
use rusty_edit::authoring::rendering::post_processing as vc_ops;
use rusty_scene::Scene;

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
