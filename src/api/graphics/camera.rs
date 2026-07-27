//! src/api/graphics/camera.rs — the `Graphics` knobs that live on the **camera**
//! rather than on a visual-correction volume: motion blur and FXAA.
//!
//! Split out of `mod.rs` when it crossed the size cap, along the seam the namespace
//! already had — the volume knobs read `VisualCorrectionComponent`, these read
//! `CameraComponent`, and the two groups share nothing but the `Graphics` table.
//!
//! Both route through the shared `authoring::camera` ops the editor's Camera card
//! calls, so the panel and the binding are one write path (#287).

use std::cell::RefCell;

use super::state::{with_cam, with_cam_mut};
use crate::api::{put, Reg};
use crate::scene::authoring::camera as camera_ops;
use crate::scene::Scene;

/// Camera motion blur: active + sample count over the active camera component.
pub(super) fn register_motion_blur<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
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
            // `2..=32` is an API-input clamp the editor card doesn't apply (it forces
            // 64), so it stays here; the op is a plain set, keeping both identical.
            with_cam_mut(scene, |e| {
                camera_ops::set_motion_blur_samples(e, n.clamp(2, 32))
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "GetMotionBlurSamples",
        scope.create_function(|_, ()| Ok(with_cam(scene, |c| c.motion_blur_samples).unwrap_or(0))),
    )
}

/// FXAA: the anti-aliasing pass at the end of the post-FX chain (#360), over the
/// active camera component — the same write path the editor's Camera card uses.
pub(super) fn register_fxaa<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
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
        // `true` with no active camera, matching what the renderer actually does in
        // that case (`postfx::params::fxaa_enabled`) — the getter must not claim FXAA
        // is off while the frame is being anti-aliased.
        scope.create_function(|_, ()| Ok(with_cam(scene, |c| c.fxaa_active).unwrap_or(true))),
    )
}
