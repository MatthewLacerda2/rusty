//! `Graphics.{Get,Set}FxaaActive` (#360): a **camera** effect, on the main camera's
//! `CameraComponent`, through the `authoring::rendering::camera` op the Camera card
//! calls (#287).

use super::super::state::{with_cam, with_cam_mut};
use crate::api::ApiCell;
use crate::api::{put, Reg};
use rusty_scene::authoring::rendering::camera as camera_ops;
use rusty_scene::Scene;

/// FXAA: the anti-aliasing pass at the end of the post-FX chain (#360), over the
/// main camera's component — the same write path the editor's Camera card uses.
pub(super) fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "SetFxaaActive",
        lua.create_function(move |lua, active: bool| {
            with_cam_mut(&*scene.get(lua)?, |e| {
                camera_ops::set_fxaa_active(e, active)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "GetFxaaActive",
        // `true` without a screen camera, matching what the renderer does for a view
        // with no camera (`postfx::params::fxaa_enabled` with `None`): the getter must
        // not claim FXAA is off while the frame is being anti-aliased.
        lua.create_function(move |lua, ()| {
            Ok(with_cam(&*scene.get(lua)?, |c| c.fxaa_active).unwrap_or(true))
        }),
    )
}
