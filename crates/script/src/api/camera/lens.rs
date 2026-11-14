//! crates/script/src/api/camera/lens.rs — the per-entity lens and stacking functions (#1341):
//! field of view (always vertical) and the axis it is shown along (#2038), clip
//! planes, render order and clear flags, Unity's `fieldOfView`,
//! `nearClipPlane` / `farClipPlane`, `depth` and `clearFlags`; plus Unity's two
//! pure FOV conversions, `VerticalToHorizontalFov` / `HorizontalToVerticalFov`.
//! Registered by `component::register`, with its `read` / `write` contract: getters
//! return `nil` without a camera, setters are then no-ops. The lens getters return
//! the held lens the camera draws with (`scene::camera::held_lens`, #1359), as
//! Unity's do, so a value loaded out of bounds reads back within them.

use super::component::{read, write};
use crate::api::entity_arg::EntityArg;
use crate::api::ApiCell;
use crate::api::{put, Reg};
use rusty_components::{
    horizontal_to_vertical_fov, vertical_to_horizontal_fov, ClearFlags, FovAxis,
};
use rusty_scene::authoring::rendering::camera as camera_ops;
use rusty_scene::camera::held_lens;
use rusty_scene::Scene;

/// Register `Get/SetFieldOfView`, `Get/SetFovAxis`, the FOV conversions,
/// `Get/SetClipPlanes`, `Get/SetRenderOrder` and `Get/SetClearFlags` onto the
/// `Camera` table.
pub(super) fn register(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetFieldOfView",
        lua.create_function(move |lua, id: EntityArg| {
            Ok(read(&*scene.get(lua)?, id.read(), |c| held_lens(c).fov))
        }),
    )?;
    put(
        table,
        "SetFieldOfView",
        lua.create_function(move |lua, (id, fov): (EntityArg, f32)| {
            let id = id.live("Camera", "SetFieldOfView")?;
            write(&*scene.get(lua)?, id, |c| camera_ops::set_fov(c, fov));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetClipPlanes",
        lua.create_function(move |lua, id: EntityArg| {
            let (near, far): (Option<f32>, Option<f32>) = read(&*scene.get(lua)?, id.read(), |c| {
                let lens = held_lens(c);
                (lens.near, lens.far)
            })
            .unzip();
            Ok((near, far))
        }),
    )?;
    put(
        table,
        "SetClipPlanes",
        lua.create_function(move |lua, (id, near, far): (EntityArg, f32, f32)| {
            let id = id.live("Camera", "SetClipPlanes")?;
            write(&*scene.get(lua)?, id, |c| {
                camera_ops::set_clip_planes(c, near, far)
            });
            Ok(())
        }),
    )?;
    register_axis(lua, table, scene)?;
    register_stacking(lua, table, scene)
}

/// `Get/SetFovAxis` by name (display only: the stored fov stays vertical; an
/// unknown name is an error naming the choices) and Unity's two pure conversions.
fn register_axis(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetFovAxis",
        lua.create_function(move |lua, id: EntityArg| {
            Ok(read(&*scene.get(lua)?, id.read(), |c| c.fov_axis.name()))
        }),
    )?;
    put(
        table,
        "SetFovAxis",
        lua.create_function(move |lua, (id, name): (EntityArg, String)| {
            let id = id.live("Camera", "SetFovAxis")?;
            let axis = FovAxis::parse(&name).ok_or_else(|| {
                mlua::Error::RuntimeError(format!(
                    "SetFovAxis: unknown fov axis {name:?} (want one of {})",
                    FovAxis::names()
                ))
            })?;
            write(&*scene.get(lua)?, id, |c| camera_ops::set_fov_axis(c, axis));
            Ok(())
        }),
    )?;
    put(
        table,
        "VerticalToHorizontalFov",
        lua.create_function(|_, (fov, aspect): (f32, f32)| {
            Ok(vertical_to_horizontal_fov(fov, aspect))
        }),
    )?;
    put(
        table,
        "HorizontalToVerticalFov",
        lua.create_function(|_, (fov, aspect): (f32, f32)| {
            Ok(horizontal_to_vertical_fov(fov, aspect))
        }),
    )
}

/// `Get/SetRenderOrder` (Unity's `depth`: higher draws later, on top) and
/// `Get/SetClearFlags` by name; an unknown name is an error naming the choices.
fn register_stacking(lua: &mlua::Lua, table: &mlua::Table, scene: ApiCell<Scene>) -> Reg {
    put(
        table,
        "GetRenderOrder",
        lua.create_function(move |lua, id: EntityArg| {
            Ok(read(&*scene.get(lua)?, id.read(), |c| c.render_order))
        }),
    )?;
    put(
        table,
        "SetRenderOrder",
        lua.create_function(move |lua, (id, order): (EntityArg, i32)| {
            let id = id.live("Camera", "SetRenderOrder")?;
            write(&*scene.get(lua)?, id, |c| {
                camera_ops::set_render_order(c, order)
            });
            Ok(())
        }),
    )?;
    put(
        table,
        "GetClearFlags",
        lua.create_function(move |lua, id: EntityArg| {
            Ok(read(&*scene.get(lua)?, id.read(), |c| c.clear_flags.name()))
        }),
    )?;
    put(
        table,
        "SetClearFlags",
        lua.create_function(move |lua, (id, name): (EntityArg, String)| {
            let id = id.live("Camera", "SetClearFlags")?;
            let flags = ClearFlags::parse(&name).ok_or_else(|| {
                mlua::Error::RuntimeError(format!(
                    "SetClearFlags: unknown clear flags {name:?} (want one of {})",
                    ClearFlags::names()
                ))
            })?;
            write(&*scene.get(lua)?, id, |c| {
                camera_ops::set_clear_flags(c, flags)
            });
            Ok(())
        }),
    )
}
