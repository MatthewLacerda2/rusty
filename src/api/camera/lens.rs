//! src/api/camera/lens.rs — the per-entity lens and stacking functions (#1341):
//! field of view, clip planes, render order and clear flags, Unity's
//! `fieldOfView`, `nearClipPlane` / `farClipPlane`, `depth` and `clearFlags`.
//! Registered by `component::register`, with its `read` / `write` contract: getters
//! return `nil` without a camera, setters are then no-ops. The lens getters return
//! the held lens the camera draws with (`scene::camera::held_lens`, #1359), as
//! Unity's do, so a value loaded out of bounds reads back within them.

use std::cell::RefCell;

use super::component::{read, write};
use crate::api::entity_arg::EntityArg;
use crate::api::{put, Reg};
use crate::components::ClearFlags;
use crate::scene::authoring::camera as camera_ops;
use crate::scene::camera::held_lens;
use crate::scene::Scene;

/// Register `Get/SetFieldOfView`, `Get/SetClipPlanes`, `Get/SetRenderOrder` and
/// `Get/SetClearFlags` onto the `Camera` table.
pub(super) fn register<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GetFieldOfView",
        scope.create_function(|_, id: EntityArg| Ok(read(scene, id.read(), |c| held_lens(c).fov))),
    )?;
    put(
        table,
        "SetFieldOfView",
        scope.create_function(|_, (id, fov): (EntityArg, f32)| {
            let id = id.live("Camera", "SetFieldOfView")?;
            write(scene, id, |c| camera_ops::set_fov(c, fov));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetClipPlanes",
        scope.create_function(|_, id: EntityArg| {
            let (near, far): (Option<f32>, Option<f32>) = read(scene, id.read(), |c| {
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
        scope.create_function(|_, (id, near, far): (EntityArg, f32, f32)| {
            let id = id.live("Camera", "SetClipPlanes")?;
            write(scene, id, |c| camera_ops::set_clip_planes(c, near, far));
            Ok(())
        }),
    )?;
    register_stacking(scope, table, scene)
}

/// `Get/SetRenderOrder` (Unity's `depth`: higher draws later, on top) and
/// `Get/SetClearFlags` by name; an unknown name is an error naming the choices.
fn register_stacking<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "GetRenderOrder",
        scope.create_function(|_, id: EntityArg| Ok(read(scene, id.read(), |c| c.render_order))),
    )?;
    put(
        table,
        "SetRenderOrder",
        scope.create_function(|_, (id, order): (EntityArg, i32)| {
            let id = id.live("Camera", "SetRenderOrder")?;
            write(scene, id, |c| camera_ops::set_render_order(c, order));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetClearFlags",
        scope.create_function(|_, id: EntityArg| {
            Ok(read(scene, id.read(), |c| c.clear_flags.name()))
        }),
    )?;
    put(
        table,
        "SetClearFlags",
        scope.create_function(|_, (id, name): (EntityArg, String)| {
            let id = id.live("Camera", "SetClearFlags")?;
            let flags = ClearFlags::parse(&name).ok_or_else(|| {
                mlua::Error::RuntimeError(format!(
                    "SetClearFlags: unknown clear flags {name:?} (want one of {})",
                    ClearFlags::names()
                ))
            })?;
            write(scene, id, |c| camera_ops::set_clear_flags(c, flags));
            Ok(())
        }),
    )
}
