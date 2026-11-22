//! Each write in `with_vc_mut` / `with_cam_mut` calls an `edit::authoring` op, so the Inspector
//! and `Graphics.*` validate alike.

use std::cell::RefCell;

use rusty_components::{CameraComponent, PostProcessingComponent};
use rusty_scene::camera::{camera_volume, main_camera};
use rusty_scene::Scene;

fn active_vc_id(scene: &Scene) -> Option<u32> {
    main_camera(scene).filter(|&id| camera_volume(scene, id).is_some())
}

fn active_cam_id(scene: &Scene) -> Option<u32> {
    main_camera(scene)
}

pub(super) fn with_vc<R>(
    scene: &RefCell<Scene>,
    f: impl FnOnce(&PostProcessingComponent) -> R,
) -> Option<R> {
    let scene = scene.borrow();
    let id = active_vc_id(&scene)?;
    scene.world.post_processing(id).map(|vc| f(&vc))
}

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

pub(super) fn with_cam<R>(
    scene: &RefCell<Scene>,
    f: impl FnOnce(&CameraComponent) -> R,
) -> Option<R> {
    let scene = scene.borrow();
    let id = active_cam_id(&scene)?;
    scene.world.camera(id).map(|c| f(&c))
}

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
