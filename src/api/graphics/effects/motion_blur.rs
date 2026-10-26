//! `Graphics.{Get,Set}MotionBlur*` (#1436): a **camera** effect, its knobs on the
//! main camera's `CameraComponent` (Unity's camera property), not on a volume,
//! through the `authoring::rendering::camera` ops the Camera card calls (#287).

use std::cell::RefCell;

use super::super::state::{with_cam, with_cam_mut};
use crate::api::{put, Reg};
use crate::scene::authoring::rendering::camera as camera_ops;
use crate::scene::Scene;

/// Camera motion blur: active, sample count, intensity and clamp (#1436) over the
/// main camera's component. Without a screen camera nothing blurs, so every getter
/// reports "off": `false` and `0`, never a new camera's defaults.
pub(super) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetMotionBlurActive",
        scope.create_function(|_, active: bool| {
            with_cam_mut(scene, |e| camera_ops::set_motion_blur_active(e, active));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetMotionBlurActive",
        scope.create_function(|_, ()| {
            Ok(with_cam(scene, |c| c.motion_blur_active).unwrap_or(false))
        }),
    )?;

    put(
        table,
        "SetMotionBlurSamples",
        scope.create_function(|_, n: u32| {
            with_cam_mut(scene, |e| camera_ops::set_motion_blur_samples(e, n));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetMotionBlurSamples",
        scope.create_function(|_, ()| Ok(with_cam(scene, |c| c.motion_blur_samples).unwrap_or(0))),
    )?;
    register_shape(scope, table, scene)
}

/// Motion blur's intensity (the shutter fraction) and clamp (the longest streak, a
/// fraction of the view's height), #1436.
fn register_shape<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetMotionBlurIntensity",
        scope.create_function(|_, v: f32| {
            with_cam_mut(scene, |e| camera_ops::set_motion_blur_intensity(e, v));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetMotionBlurIntensity",
        scope.create_function(|_, ()| {
            Ok(with_cam(scene, |c| c.motion_blur_intensity).unwrap_or(0.0))
        }),
    )?;
    put(
        table,
        "SetMotionBlurMaxLength",
        scope.create_function(|_, v: f32| {
            with_cam_mut(scene, |e| camera_ops::set_motion_blur_max_length(e, v));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetMotionBlurMaxLength",
        scope.create_function(|_, ()| {
            Ok(with_cam(scene, |c| c.motion_blur_max_length).unwrap_or(0.0))
        }),
    )
}
