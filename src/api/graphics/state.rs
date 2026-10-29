//! src/api/graphics/state.rs — backing-store accessors for the `Graphics` namespace.
//!
//! Locate the screen's camera, `scene::camera::main_camera` (Unity's `Camera.main`,
//! the base of the screen stack, #1434), and its post-FX volume, and read or mutate
//! them, and the render scale in the shared graphics cell.
//! These are the only places `Graphics.*` reaches into render-only component state.

use std::cell::RefCell;

use super::super::{put, Reg};
use crate::components::{CameraComponent, PostProcessingComponent};
use crate::core::settings::render_scale::RenderScale;
use crate::core::settings::GraphicsSettings;
use crate::scene::camera::{camera_volume, main_camera};
use crate::scene::Scene;

/// The scene's render scale (#1498), in the shared graphics cell: clamped to URP's
/// range.
pub(super) fn register_render_scale<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    graphics: &'scope RefCell<GraphicsSettings>,
) -> Reg {
    put(
        table,
        "GetRenderScale",
        scope.create_function(|_, ()| Ok(graphics.borrow().render_scale.get())),
    )?;
    put(
        table,
        "SetRenderScale",
        scope.create_function(|_, scale: f32| {
            graphics.borrow_mut().render_scale = RenderScale::new(scale);
            Ok(())
        }),
    )
}

/// The main camera, when its post-processing volume is active: the volume the
/// screen's post-FX reads.
fn active_vc_id(scene: &Scene) -> Option<u32> {
    main_camera(scene).filter(|&id| camera_volume(scene, id).is_some())
}

/// The main camera: the screen stack's base.
fn active_cam_id(scene: &Scene) -> Option<u32> {
    main_camera(scene)
}

/// Run `f` on the main camera's active post-processing volume, returning its result.
pub(super) fn with_vc<R>(
    scene: &RefCell<Scene>,
    f: impl FnOnce(&PostProcessingComponent) -> R,
) -> Option<R> {
    let scene = scene.borrow();
    let id = active_vc_id(&scene)?;
    scene.world.post_processing(id).map(|vc| f(&vc))
}

/// Mutate the main camera's active post-processing volume (no-op without one).
/// The caller routes the write through a shared
/// `authoring::rendering::post_processing::*` op (#287).
pub(super) fn with_vc_mut(scene: &RefCell<Scene>, f: impl FnOnce(&mut PostProcessingComponent)) {
    let mut scene = scene.borrow_mut();
    let Some(id) = active_vc_id(&scene) else {
        return;
    };
    let guard = scene.world.post_processing_mut(id);
    if let Some(mut vc) = guard {
        f(&mut vc);
    }
}

/// Run `f` on the main camera's component, returning its result.
pub(super) fn with_cam<R>(
    scene: &RefCell<Scene>,
    f: impl FnOnce(&CameraComponent) -> R,
) -> Option<R> {
    let scene = scene.borrow();
    let id = active_cam_id(&scene)?;
    scene.world.camera(id).map(|c| f(&c))
}

/// Mutate the main camera's component (no-op without a screen camera). The
/// caller routes the write through a shared `authoring::rendering::camera::*` op (#287).
pub(super) fn with_cam_mut(scene: &RefCell<Scene>, f: impl FnOnce(&mut CameraComponent)) {
    let mut scene = scene.borrow_mut();
    let Some(id) = active_cam_id(&scene) else {
        return;
    };
    let guard = scene.world.camera_mut(id);
    if let Some(mut c) = guard {
        f(&mut c);
    }
}
