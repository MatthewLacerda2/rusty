//! `Graphics.{Get,Set}FxaaActive` (#360): a **camera** effect, on the main camera's
//! `CameraComponent`, through the `authoring::rendering::camera` op the Camera card
//! calls (#287).

use std::cell::RefCell;

use super::super::state::{with_cam, with_cam_mut};
use crate::api::{put, Reg};
use crate::scene::authoring::rendering::camera as camera_ops;
use crate::scene::Scene;

/// FXAA: the anti-aliasing pass at the end of the post-FX chain (#360), over the
/// main camera's component — the same write path the editor's Camera card uses.
pub(super) fn register<'scope>(
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
