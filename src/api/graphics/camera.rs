//! src/api/graphics/camera.rs — the `Graphics` knobs that live on the **camera**
//! rather than on a post-processing volume: motion blur and FXAA.
//!
//! Split out of `mod.rs` when it crossed the size cap, along the seam the namespace
//! already had — the volume knobs read `PostProcessingComponent`, these read
//! `CameraComponent`, and the two groups share nothing but the `Graphics` table.
//!
//! Both route through the shared `authoring::rendering::camera` ops the editor's Camera card
//! calls, so the panel and the binding are one write path (#287).

use std::cell::RefCell;

use super::state::{with_cam, with_cam_mut};
use crate::api::{put, Reg};
use crate::components::CameraComponent;
use crate::scene::authoring::rendering::camera as camera_ops;
use crate::scene::Scene;

/// Camera motion blur: active, sample count, intensity and clamp (#1436) over the
/// main camera's component.
pub(super) fn register_motion_blur<'scope>(
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
    register_motion_blur_shape(scope, table, scene)
}

/// Motion blur's intensity (the shutter fraction) and clamp (the longest streak, a
/// fraction of the view's height), #1436. Without a screen camera the getters return
/// a new camera's defaults.
fn register_motion_blur_shape<'scope>(
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
            let default = CameraComponent::DEFAULT_MOTION_BLUR_INTENSITY;
            Ok(with_cam(scene, |c| c.motion_blur_intensity).unwrap_or(default))
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
            let default = CameraComponent::DEFAULT_MOTION_BLUR_MAX_LENGTH;
            Ok(with_cam(scene, |c| c.motion_blur_max_length).unwrap_or(default))
        }),
    )
}

/// FXAA: the anti-aliasing pass at the end of the post-FX chain (#360), over the
/// main camera's component — the same write path the editor's Camera card uses.
pub(super) fn register_fxaa<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetFxaaActive",
        scope.create_function(|_, active: bool| {
            with_cam_mut(scene, |e| camera_ops::set_fxaa_active(e, active));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetFxaaActive",
        // `true` without a screen camera, matching what the renderer does for a view
        // with no camera (`postfx::params::fxaa_enabled` with `None`): the getter must
        // not claim FXAA is off while the frame is being anti-aliased.
        scope.create_function(|_, ()| Ok(with_cam(scene, |c| c.fxaa_active).unwrap_or(true))),
    )
}
