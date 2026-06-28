//! src/api/graphics/state.rs — backing-store accessors for the `Graphics` namespace.
//!
//! Locate the first **active** post-FX volume / camera in the scene and read or
//! mutate it, plus the small string<->enum conversions the Lua surface speaks in.
//! These are the only places `Graphics.*` reaches into render-only component state.

use std::cell::RefCell;

use super::super::{put, Reg};
use crate::components::{CameraComponent, Entity, Tonemap, VisualCorrectionComponent};
use crate::render::postfx::QualityPreset;
use crate::scene::Scene;

/// Global quality preset value get/set over the shared resource cell. Lives here (not
/// `mod.rs`) to keep that file under the size cap; reaches `quality_name`/`parse_quality`
/// directly.
pub(super) fn register_quality<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    quality: &'scope RefCell<QualityPreset>,
) -> Reg {
    put(
        table,
        "GetQuality",
        scope.create_function(|_, ()| Ok(quality_name(*quality.borrow()).to_string())),
    )?;
    put(
        table,
        "SetQuality",
        scope.create_function(|_, name: String| {
            if let Some(p) = parse_quality(&name) {
                *quality.borrow_mut() = p;
            }
            Ok(())
        }),
    )
}

/// Run `f` on the first active visual-correction volume, returning its result.
pub(super) fn with_vc<R>(
    scene: &RefCell<Scene>,
    f: impl FnOnce(&VisualCorrectionComponent) -> R,
) -> Option<R> {
    let scene = scene.borrow();
    let entity = scene
        .iter()
        .find(|e| e.active && e.visual_correction.as_ref().is_some_and(|vc| vc.active))?;
    let result = entity.visual_correction.as_ref().map(f);
    drop(entity);
    result
}

/// Mutate the entity owning the first active visual-correction volume (no-op when
/// there is none). The closure runs against the whole `Entity` so the caller routes
/// the write through a shared `authoring::visual_correction::*` op (#287).
pub(super) fn with_vc_mut(scene: &RefCell<Scene>, f: impl FnOnce(&mut Entity)) {
    let mut scene = scene.borrow_mut();
    let target = scene
        .iter()
        .find(|e| e.active && e.visual_correction.as_ref().is_some_and(|vc| vc.active))
        .map(|e| e.id);
    let Some(id) = target else { return };
    let Some(mut e) = scene.get_entity_mut(id) else {
        return;
    };
    f(&mut e);
}

/// Run `f` on the first active camera component, returning its result.
pub(super) fn with_cam<R>(
    scene: &RefCell<Scene>,
    f: impl FnOnce(&CameraComponent) -> R,
) -> Option<R> {
    let scene = scene.borrow();
    let entity = scene
        .iter()
        .find(|e| e.active && e.camera.as_ref().is_some_and(|c| c.active))?;
    let result = entity.camera.as_ref().map(f);
    drop(entity);
    result
}

/// Mutate the entity owning the first active camera component (no-op when there is
/// none). The closure runs against the whole `Entity` so the caller routes the write
/// through a shared `authoring::camera::*` op (#287).
pub(super) fn with_cam_mut(scene: &RefCell<Scene>, f: impl FnOnce(&mut Entity)) {
    let mut scene = scene.borrow_mut();
    let target = scene
        .iter()
        .find(|e| e.active && e.camera.as_ref().is_some_and(|c| c.active))
        .map(|e| e.id);
    let Some(id) = target else { return };
    let Some(mut e) = scene.get_entity_mut(id) else {
        return;
    };
    f(&mut e);
}

pub(super) fn parse_tonemap(name: &str) -> Option<Tonemap> {
    match name.to_ascii_lowercase().as_str() {
        "none" => Some(Tonemap::None),
        "reinhard" => Some(Tonemap::Reinhard),
        "aces" => Some(Tonemap::Aces),
        _ => None,
    }
}

pub(super) fn tonemap_name(tm: Tonemap) -> &'static str {
    match tm {
        Tonemap::None => "None",
        Tonemap::Reinhard => "Reinhard",
        Tonemap::Aces => "Aces",
    }
}

pub(super) fn parse_quality(name: &str) -> Option<QualityPreset> {
    match name.to_ascii_lowercase().as_str() {
        "low" => Some(QualityPreset::Low),
        "medium" => Some(QualityPreset::Medium),
        "high" => Some(QualityPreset::High),
        _ => None,
    }
}

pub(super) fn quality_name(p: QualityPreset) -> &'static str {
    match p {
        QualityPreset::Low => "Low",
        QualityPreset::Medium => "Medium",
        QualityPreset::High => "High",
    }
}
