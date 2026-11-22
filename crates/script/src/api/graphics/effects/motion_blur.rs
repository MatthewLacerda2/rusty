use super::super::state::{with_cam, with_cam_mut};
use super::knobs::parse_quality;
use crate::api::ApiCell;
use crate::api::{put, Reg};
use rusty_edit::authoring::rendering::camera as camera_ops;
use rusty_scene::Scene;

pub(super) fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetMotionBlurActive",
        lua.create_function(move |lua, active: bool| {
            with_cam_mut(&*scene.get(lua)?, |e| {
                camera_ops::set_motion_blur_active(e, active)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "GetMotionBlurActive",
        lua.create_function(move |lua, ()| {
            Ok(with_cam(&*scene.get(lua)?, |c| c.motion_blur_active).unwrap_or(false))
        }),
    )?;

    put(
        table,
        "SetMotionBlurQuality",
        lua.create_function(move |lua, level: String| {
            let quality = parse_quality("SetMotionBlurQuality", &level)?;
            with_cam_mut(&*scene.get(lua)?, |e| {
                camera_ops::set_motion_blur_quality(e, quality)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "GetMotionBlurQuality",
        lua.create_function(move |lua, ()| {
            let quality =
                with_cam(&*scene.get(lua)?, |c| c.motion_blur_quality).unwrap_or_default();
            Ok(quality.name())
        }),
    )?;
    register_samples(lua, table, scene)?;
    register_shape(lua, table, scene)
}

fn register_samples(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetMotionBlurSamples",
        lua.create_function(move |lua, n: u32| {
            with_cam_mut(&*scene.get(lua)?, |e| {
                camera_ops::set_motion_blur_samples(e, n)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "GetMotionBlurSamples",
        lua.create_function(move |lua, ()| {
            Ok(with_cam(&*scene.get(lua)?, |c| c.motion_blur_samples).unwrap_or(0))
        }),
    )
}

fn register_shape(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetMotionBlurIntensity",
        lua.create_function(move |lua, v: f32| {
            with_cam_mut(&*scene.get(lua)?, |e| {
                camera_ops::set_motion_blur_intensity(e, v)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "GetMotionBlurIntensity",
        lua.create_function(move |lua, ()| {
            Ok(with_cam(&*scene.get(lua)?, |c| c.motion_blur_intensity).unwrap_or(0.0))
        }),
    )?;
    put(
        table,
        "SetMotionBlurMaxLength",
        lua.create_function(move |lua, v: f32| {
            with_cam_mut(&*scene.get(lua)?, |e| {
                camera_ops::set_motion_blur_max_length(e, v)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "GetMotionBlurMaxLength",
        lua.create_function(move |lua, ()| {
            Ok(with_cam(&*scene.get(lua)?, |c| c.motion_blur_max_length).unwrap_or(0.0))
        }),
    )
}
