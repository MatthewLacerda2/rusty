//! `Graphics.{Get,Set}Taa*` (#1702, its knobs #1847): a **camera** effect, on the
//! main camera's `CameraComponent`, through the `authoring::rendering::camera` ops
//! the Camera card calls (#287). The float knobs come from the camera's knob table
//! (`camera::TAA_KNOBS`), so these functions, the Inspector sliders and the
//! `Graphics.taa.<knob>` tween paths reach one op.

use super::super::state::{with_cam, with_cam_mut};
use crate::api::ApiCell;
use crate::api::{put, Reg};
use rusty_components::TaaQuality;
use rusty_scene::authoring::default_camera;
use rusty_scene::authoring::rendering::camera as camera_ops;
use rusty_scene::Scene;

/// TAA: temporal anti-aliasing (#1702) and its knobs (#1847) over the main camera's
/// component, the same write path the editor's Camera card uses. Without a screen
/// camera nothing runs it, so the getters report a new camera's defaults (and
/// `GetTaaActive` reports `false`).
pub(super) fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetTaaActive",
        lua.create_function(move |lua, active: bool| {
            with_cam_mut(&*scene.get(lua)?, |e| camera_ops::set_taa_active(e, active));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetTaaActive",
        lua.create_function(move |lua, ()| {
            Ok(with_cam(&*scene.get(lua)?, |c| c.taa_active).unwrap_or(false))
        }),
    )?;
    register_quality(lua, table, scene)?;
    register_knobs(lua, table, scene)
}

/// `Set/GetTaaQuality(level)`: URP's `VeryLow`…`VeryHigh`, an unknown one refused.
fn register_quality(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetTaaQuality",
        lua.create_function(move |lua, level: String| {
            let quality = TaaQuality::parse(&level).ok_or_else(|| {
                mlua::Error::RuntimeError(format!(
                    "Graphics.SetTaaQuality: unknown quality '{level}' (one of {})",
                    TaaQuality::names()
                ))
            })?;
            with_cam_mut(&*scene.get(lua)?, |e| {
                camera_ops::set_taa_quality(e, quality)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "GetTaaQuality",
        lua.create_function(move |lua, ()| {
            let quality = with_cam(&*scene.get(lua)?, |c| c.taa_quality).unwrap_or_default();
            Ok(quality.name())
        }),
    )
}

/// `Set/Get<api>(value)` for each float knob, through its op, so the function, the
/// slider and the tween path clamp the same way. Without a screen camera the getter
/// reports a new camera's value.
fn register_knobs(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    for knob in camera_ops::TAA_KNOBS {
        put(
            table,
            &format!("Set{}", knob.api),
            lua.create_function(move |lua, value: f32| {
                with_cam_mut(&*scene.get(lua)?, |e| (knob.set)(e, value));
                Ok(())
            }),
        )?;
        put(
            table,
            &format!("Get{}", knob.api),
            lua.create_function(move |lua, ()| {
                Ok(with_cam(&*scene.get(lua)?, knob.get)
                    .unwrap_or_else(|| (knob.get)(&default_camera())))
            }),
        )?;
    }
    Ok(())
}
