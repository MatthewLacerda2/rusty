//! `Graphics.{Get,Set}TaaActive` (#1702): a **camera** effect, on the main camera's
//! `CameraComponent`, through the `authoring::rendering::camera` op the Camera card
//! calls (#287).

use super::super::state::{with_cam, with_cam_mut};
use crate::api::ApiCell;
use crate::api::{put, Reg};
use crate::scene::authoring::rendering::camera as camera_ops;
use crate::scene::Scene;

/// TAA: temporal anti-aliasing (#1702) over the main camera's component, the same
/// write path the editor's Camera card uses. Without a screen camera nothing runs
/// it, so the getter reports `false`.
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
    )
}
