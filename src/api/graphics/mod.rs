//! src/api/graphics.rs — `Graphics` namespace.
//!
//! Live get/set over the engine's EXISTING post-FX and quality knobs, so a script
//! can drive its own settings logic in Lua. Two backing stores:
//!
//! * Per-volume render state — the active entity's `VisualCorrectionComponent`
//!   (bloom, exposure/contrast/saturation/gamma, tonemap, SSR), registered here,
//!   plus its `CameraComponent` knobs (motion blur, FXAA) in the `camera` submodule.
//!   `build_post_params` rebuilds the GPU uniform from these EVERY frame, so a write
//!   here takes effect next frame for free — no GPU pipeline is touched from script.
//! * The global `QualityPreset` resource (Low/Medium/High), gating SSR + motion
//!   blur. A plain value get/set (`register_quality`, in `state`); the platform layer
//!   reads the shared cell each frame and hands it to `renderer.set_quality`.
//!
//! Every write is ONE-WAY into render-only state: it never feeds back into
//! `FixedUpdate`, so toggling these knobs can't change the deterministic sim. The
//! per-volume setters route through the shared `authoring::{camera,
//! visual_correction}` ops the editor cards use, so panel and binding never drift.

use std::cell::RefCell;

mod camera;
mod state;

use mlua::Lua;

use self::state::{parse_tonemap, register_quality, tonemap_name, with_vc, with_vc_mut};
use super::{put, Reg};
use crate::render::postfx::QualityPreset;
use crate::scene::authoring::visual_correction as vc_ops;
use crate::scene::Scene;

/// Register the `Graphics` namespace onto `lua`.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    scene: &'scope RefCell<Scene>,
    quality: &'scope RefCell<QualityPreset>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_bloom(scope, &table, scene)?;
    register_exposure_contrast(scope, &table, scene)?;
    register_saturation_gamma(scope, &table, scene)?;
    register_tonemap(scope, &table, scene)?;
    register_ssr(scope, &table, scene)?;
    camera::register_motion_blur(scope, &table, scene)?;
    camera::register_fxaa(scope, &table, scene)?;
    register_quality(scope, &table, quality)?;

    lua.globals()
        .set("Graphics", table)
        .map_err(|e| e.to_string())
}

/// Bloom: active / intensity / threshold over the active visual-correction volume.
fn register_bloom<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetBloomActive",
        scope.create_function(|_, active: bool| {
            with_vc_mut(scene, |e| vc_ops::set_bloom_active(e, active));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetBloomActive",
        scope.create_function(|_, ()| Ok(with_vc(scene, |vc| vc.bloom_active).unwrap_or(false))),
    )?;

    put(
        table,
        "SetBloomIntensity",
        scope.create_function(|_, v: f32| {
            with_vc_mut(scene, |e| vc_ops::set_bloom_intensity(e, v));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetBloomIntensity",
        scope.create_function(|_, ()| Ok(with_vc(scene, |vc| vc.bloom_intensity).unwrap_or(0.0))),
    )?;

    put(
        table,
        "SetBloomThreshold",
        scope.create_function(|_, v: f32| {
            with_vc_mut(scene, |e| vc_ops::set_bloom_threshold(e, v));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetBloomThreshold",
        scope.create_function(|_, ()| Ok(with_vc(scene, |vc| vc.bloom_threshold).unwrap_or(0.0))),
    )
}

/// Color grading: `Set`/`Get` for exposure and contrast.
fn register_exposure_contrast<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetExposure",
        scope.create_function(|_, v: f32| {
            with_vc_mut(scene, |e| vc_ops::set_exposure(e, v));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetExposure",
        scope.create_function(|_, ()| Ok(with_vc(scene, |vc| vc.exposure).unwrap_or(0.0))),
    )?;

    put(
        table,
        "SetContrast",
        scope.create_function(|_, v: f32| {
            with_vc_mut(scene, |e| vc_ops::set_contrast(e, v));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetContrast",
        scope.create_function(|_, ()| Ok(with_vc(scene, |vc| vc.contrast).unwrap_or(1.0))),
    )
}

/// Color grading: `Set`/`Get` for saturation and gamma.
fn register_saturation_gamma<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetSaturation",
        scope.create_function(|_, v: f32| {
            with_vc_mut(scene, |e| vc_ops::set_saturation(e, v));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetSaturation",
        scope.create_function(|_, ()| Ok(with_vc(scene, |vc| vc.saturation).unwrap_or(1.0))),
    )?;

    put(
        table,
        "SetGamma",
        scope.create_function(|_, v: f32| {
            with_vc_mut(scene, |e| vc_ops::set_gamma(e, v));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetGamma",
        scope.create_function(|_, ()| Ok(with_vc(scene, |vc| vc.gamma).unwrap_or(2.2))),
    )
}

/// Tonemap operator: `SetTonemap` / `GetTonemap` (defaulting to Aces).
fn register_tonemap<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetTonemap",
        scope.create_function(|_, name: String| {
            if let Some(tm) = parse_tonemap(&name) {
                with_vc_mut(scene, |e| vc_ops::set_tonemap(e, tm));
            }
            Ok(())
        }),
    )?;
    put(
        table,
        "GetTonemap",
        scope.create_function(|_, ()| {
            Ok(with_vc(scene, |vc| tonemap_name(vc.tonemap).to_string())
                .unwrap_or_else(|| "Aces".to_string()))
        }),
    )
}

/// Screen-space reflections: active + quality string (gated by the High preset).
fn register_ssr<'lua, 'scope>(
    scope: &mlua::Scope<'lua, 'scope>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetSsrActive",
        scope.create_function(|_, active: bool| {
            with_vc_mut(scene, |e| vc_ops::set_ssr_active(e, active));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetSsrActive",
        scope.create_function(|_, ()| Ok(with_vc(scene, |vc| vc.ssr_active).unwrap_or(false))),
    )?;

    put(
        table,
        "SetSsrQuality",
        scope.create_function(|_, q: String| {
            with_vc_mut(scene, |e| vc_ops::set_ssr_quality(e, q.clone()));
            Ok(())
        }),
    )?;
    put(
        table,
        "GetSsrQuality",
        scope.create_function(|_, ()| {
            Ok(with_vc(scene, |vc| vc.ssr_quality.clone()).unwrap_or_default())
        }),
    )
}
