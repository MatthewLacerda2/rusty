//! src/api/graphics/state.rs — backing-store accessors for the `Graphics` namespace.
//!
//! Locate the screen's camera, `scene::camera::main_camera` (Unity's `Camera.main`,
//! the base of the screen stack, #1434), and its post-FX volume, and read or mutate
//! them, plus the small string<->enum conversions the Lua surface speaks in.
//! These are the only places `Graphics.*` reaches into render-only component state.

use std::cell::RefCell;

use super::super::{put, Reg};
use crate::components::{CameraComponent, VisualCorrectionComponent};
use crate::core::settings::quality::QualityPreset;
use crate::scene::camera::{camera_volume, main_camera};
use crate::scene::Scene;

/// Global quality preset value get/set over the shared resource cell. Lives here (not
/// `mod.rs`) to keep that file under the size cap; reaches `QualityPreset::name`/`QualityPreset::parse`
/// directly.
pub(super) fn register_quality<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    quality: &'scope RefCell<QualityPreset>,
) -> Reg {
    put(
        table,
        "GetQuality",
        scope.create_function(|_, ()| Ok(quality.borrow().name().to_string())),
    )?;
    put(
        table,
        "SetQuality",
        scope.create_function(|_, name: String| {
            if let Some(p) = QualityPreset::parse(&name) {
                *quality.borrow_mut() = p;
            }
            Ok(())
        }),
    )
}

/// The main camera, when its visual-correction volume is active: the volume the
/// screen's post-FX reads.
fn active_vc_id(scene: &Scene) -> Option<u32> {
    main_camera(scene).filter(|&id| camera_volume(scene, id).is_some())
}

/// The main camera: the screen stack's base.
fn active_cam_id(scene: &Scene) -> Option<u32> {
    main_camera(scene)
}

/// Run `f` on the main camera's active visual-correction volume, returning its result.
pub(super) fn with_vc<R>(
    scene: &RefCell<Scene>,
    f: impl FnOnce(&VisualCorrectionComponent) -> R,
) -> Option<R> {
    let scene = scene.borrow();
    let id = active_vc_id(&scene)?;
    scene.world.visual_correction(id).map(|vc| f(&vc))
}

/// Mutate the main camera's active visual-correction volume (no-op without one).
/// The caller routes the write through a shared
/// `authoring::rendering::visual_correction::*` op (#287).
pub(super) fn with_vc_mut(scene: &RefCell<Scene>, f: impl FnOnce(&mut VisualCorrectionComponent)) {
    let mut scene = scene.borrow_mut();
    let Some(id) = active_vc_id(&scene) else {
        return;
    };
    let guard = scene.world.visual_correction_mut(id);
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
