//! src/api/graphics.rs — `Graphics` namespace.
//!
//! Live get/set over the engine's EXISTING post-FX and quality knobs, so a script
//! can drive its own settings logic in Lua. Two backing stores:
//!
//! * Per-volume render state — the main camera's `PostProcessingComponent`
//!   (bloom, contrast/saturation/gamma, tonemap, SSR), registered here, its exposure
//!   and auto-exposure (`exposure`, #1444),
//!   its colour grade (white balance, Lift / Gamma / Gain, hue shift; `grading`)
//!   its lens effects (vignette, chromatic aberration, film grain; `lens`, #1439)
//!   and its depth of field (`depth_of_field`, #1445),
//!   plus its `CameraComponent` knobs (motion blur, FXAA) in the `camera` submodule.
//!   The main camera is the screen stack's base (`scene::camera::main_camera`, Unity's
//!   `Camera.main`, #1434), the camera whose components `build_post_params` packs into
//!   the screen's GPU uniform EVERY frame, so a
//!   write here takes effect next frame for free — no GPU pipeline is touched from script.
//!   The same volume carries the sun's shadow cascades (`shadows`, #435), which the
//!   shadow pass re-fits from every frame, its ambient occlusion (`ssao`, #436) and
//!   its authored post-FX list and their runtime params (`effects`, #397, #671).
//! * The scene's fog (`fog`, #437) — scene-level, not per-volume, so it needs no
//!   volume; the renderer packs it into every pass's camera globals each frame.
//! * The global `QualityPreset` resource (Low/Medium/High), gating SSR + motion
//!   blur. A plain value get/set (`register_quality`, in `state`); the platform layer
//!   reads the shared cell each frame and hands it to `renderer.set_quality`.
//! * The player's graphics options (`options`, #1438) in the same cell, which gate
//!   every camera's effects, and the project's defaults for them on `Application`.
//!
//! Every write is ONE-WAY into render-only state: it never feeds back into
//! `FixedUpdate`, so toggling these knobs can't change the deterministic sim. The
//! per-volume setters route through the shared `authoring::{rendering::camera,
//! post_processing}` ops the editor cards use, so panel and binding never drift.

use std::cell::RefCell;

mod camera;
mod depth_of_field;
mod effects;
mod exposure;
mod fog;
mod grading;
mod lens;
mod options;
mod shadows;
mod ssao;
mod state;

use mlua::Lua;

use self::state::{register_quality, with_vc, with_vc_mut};
use super::{put, Reg};
use crate::components::Tonemap;
use crate::core::application::Application;
use crate::core::settings::GraphicsSettings;
use crate::scene::authoring::rendering::post_processing as vc_ops;
use crate::scene::Scene;

/// Register the `Graphics` namespace onto `lua`.
pub fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    scene: &'scope RefCell<Scene>,
    graphics: &'scope RefCell<GraphicsSettings>,
    app: &'scope RefCell<Application>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_bloom(scope, &table, scene)?;
    exposure::register_exposure(scope, &table, scene)?;
    register_contrast(scope, &table, scene)?;
    register_saturation_gamma(scope, &table, scene)?;
    register_tonemap(scope, &table, scene)?;
    grading::register_grading(scope, &table, scene)?;
    lens::register_lens(scope, &table, scene)?;
    depth_of_field::register_depth_of_field(scope, &table, scene)?;
    register_ssr(scope, &table, scene)?;
    camera::register_motion_blur(scope, &table, scene)?;
    camera::register_fxaa(scope, &table, scene)?;
    register_quality(scope, &table, graphics)?;
    options::register_options(scope, &table, graphics, app)?;
    shadows::register_shadows(scope, &table, scene)?;
    fog::register_fog(scope, &table, scene)?;
    ssao::register_ssao(scope, &table, scene)?;
    effects::register_custom_effects(scope, &table, scene)?;
    effects::register_post_params(scope, &table, scene)?;

    lua.globals()
        .set("Graphics", table)
        .map_err(|e| e.to_string())
}

/// Bloom: active / intensity / threshold over the active post-processing volume.
fn register_bloom<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
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
        scope.create_function(|_, ()| Ok(with_vc(scene, |vc| vc.bloom.active).unwrap_or(false))),
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
        scope.create_function(|_, ()| Ok(with_vc(scene, |vc| vc.bloom.intensity).unwrap_or(0.0))),
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
        scope.create_function(|_, ()| Ok(with_vc(scene, |vc| vc.bloom.threshold).unwrap_or(0.0))),
    )
}

/// Color grading: `Set`/`Get` for contrast.
fn register_contrast<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
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
        scope.create_function(|_, ()| {
            Ok(with_vc(scene, |vc| vc.color_adjustments.contrast).unwrap_or(1.0))
        }),
    )
}

/// Color grading: `Set`/`Get` for saturation and gamma.
fn register_saturation_gamma<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
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
        scope.create_function(|_, ()| {
            Ok(with_vc(scene, |vc| vc.color_adjustments.saturation).unwrap_or(1.0))
        }),
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
        scope.create_function(|_, ()| {
            Ok(with_vc(scene, |vc| vc.tonemapping.display_gamma).unwrap_or(1.0))
        }),
    )
}

/// Tonemap operator: `SetTonemap` / `GetTonemap` (defaulting to ACES).
fn register_tonemap<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
    table: &mlua::Table,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    put(
        table,
        "SetTonemap",
        scope.create_function(|_, name: String| {
            if let Some(tm) = Tonemap::parse(&name) {
                with_vc_mut(scene, |e| vc_ops::set_tonemap(e, tm));
            }
            Ok(())
        }),
    )?;
    put(
        table,
        "GetTonemap",
        scope.create_function(|_, ()| {
            Ok(with_vc(scene, |vc| vc.tonemapping.mode)
                .unwrap_or_default()
                .name()
                .to_string())
        }),
    )
}

/// Screen-space reflections: active + quality string (gated by the High preset).
fn register_ssr<'scope>(
    scope: &'scope mlua::Scope<'scope, '_>,
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
        scope.create_function(|_, ()| Ok(with_vc(scene, |vc| vc.ssr.active).unwrap_or(false))),
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
            Ok(with_vc(scene, |vc| vc.ssr.quality.clone()).unwrap_or_default())
        }),
    )
}
