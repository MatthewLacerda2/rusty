//! `Light`'s shadow knobs: whether a point or spot light casts shadows through the
//! shadow atlas (#468), and how far from the camera its shadow reaches (#1712).

use mlua::Lua;

use super::super::{put, Reg};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::scene::authoring::rendering::light as light_ops;
use crate::scene::Scene;

/// Register `Get/SetCastShadows` and `Get/SetShadowFadeDistance` onto `table`.
pub(super) fn register(lua: &Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    register_cast_shadows(lua, table, scene)?;
    register_fade_distance(lua, table, scene)
}

/// `GetCastShadows` / `SetCastShadows`: whether a point or spot light casts shadows
/// through the shadow atlas (#468). The sun always casts; it ignores the flag.
fn register_cast_shadows(lua: &Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetCastShadows",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            Ok(scene.world.light(id.read()).is_some_and(|l| l.cast_shadows))
        }),
    )?;

    put(
        table,
        "SetCastShadows",
        lua.create_function(move |lua, (id, on): (EntityArg, bool)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Light", "SetCastShadows")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.light_mut(id) {
                light_ops::set_cast_shadows(&mut c, on);
            }
            Ok(())
        }),
    )
}

/// `GetShadowFadeDistance` / `SetShadowFadeDistance`: the camera distance past
/// which a point or spot light's shadow is no longer sampled (#1712, HDRP's
/// Shadow Fade Distance); `0` is no fade.
fn register_fade_distance(lua: &Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetShadowFadeDistance",
        lua.create_function(move |lua, id: EntityArg| {
            let scene = &*scene.get(lua)?;
            let scene = scene.borrow();
            let light = scene.world.light(id.read());
            Ok(light.map_or(0.0, |l| l.shadow_fade_distance))
        }),
    )?;

    put(
        table,
        "SetShadowFadeDistance",
        lua.create_function(move |lua, (id, d): (EntityArg, f32)| {
            let scene = &*scene.get(lua)?;
            let id = id.live("Light", "SetShadowFadeDistance")?;
            let mut scene = scene.borrow_mut();
            if let Some(mut c) = scene.world.light_mut(id) {
                light_ops::set_shadow_fade_distance(&mut c, d);
            }
            Ok(())
        }),
    )
}
