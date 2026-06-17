//! src/api/graphics.rs — `Graphics` namespace.
//!
//! Live get/set over the engine's EXISTING post-FX and quality knobs, so a script
//! can drive its own settings logic in Lua. Two backing stores:
//!
//! * Per-volume render state — the active entity's `VisualCorrectionComponent`
//!   (bloom, exposure/contrast/saturation/gamma, tonemap, SSR) and its
//!   `CameraComponent` motion-blur fields. `build_post_params` rebuilds the GPU
//!   uniform from these EVERY frame, so a write here takes effect next frame for
//!   free — no GPU pipeline is touched from script.
//! * The global `QualityPreset` resource (Low/Medium/High), gating SSR + motion
//!   blur. Exposed as a plain value get/set; the platform layer reads the shared
//!   cell each frame and hands it to `renderer.set_quality`, which already guards
//!   the bloom-buffer reallocation a tier change needs.
//!
//! Every write is ONE-WAY into render-only state: it never feeds back into
//! `FixedUpdate`, so toggling these knobs can't change how the deterministic sim
//! evolves.

use std::cell::RefCell;
use std::rc::Rc;

mod state;

use mlua::Lua;

use self::state::{
    parse_quality, parse_tonemap, quality_name, tonemap_name, with_cam, with_cam_mut, with_vc,
    with_vc_mut,
};
use super::{put, Reg};
use crate::render::postfx::QualityPreset;
use crate::scene::Scene;

/// Register the `Graphics` namespace onto `lua`.
pub fn register(
    lua: &Lua,
    scene: &Rc<RefCell<Scene>>,
    quality: &Rc<RefCell<QualityPreset>>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    register_bloom(lua, &table, scene)?;
    register_color(lua, &table, scene)?;
    register_ssr(lua, &table, scene)?;
    register_motion_blur(lua, &table, scene)?;
    register_quality(lua, &table, quality)?;

    lua.globals()
        .set("Graphics", table)
        .map_err(|e| e.to_string())
}

/// Bloom: active / intensity / threshold over the active visual-correction volume.
fn register_bloom(lua: &Lua, table: &mlua::Table, scene: &Rc<RefCell<Scene>>) -> Reg {
    let s = Rc::clone(scene);
    put(
        table,
        "SetBloomActive",
        lua.create_function(move |_, active: bool| {
            with_vc_mut(&s, |vc| vc.bloom_active = active);
            Ok(())
        }),
    )?;
    let s = Rc::clone(scene);
    put(
        table,
        "GetBloomActive",
        lua.create_function(move |_, ()| Ok(with_vc(&s, |vc| vc.bloom_active).unwrap_or(false))),
    )?;

    let s = Rc::clone(scene);
    put(
        table,
        "SetBloomIntensity",
        lua.create_function(move |_, v: f32| {
            with_vc_mut(&s, |vc| vc.bloom_intensity = v.max(0.0));
            Ok(())
        }),
    )?;
    let s = Rc::clone(scene);
    put(
        table,
        "GetBloomIntensity",
        lua.create_function(move |_, ()| Ok(with_vc(&s, |vc| vc.bloom_intensity).unwrap_or(0.0))),
    )?;

    let s = Rc::clone(scene);
    put(
        table,
        "SetBloomThreshold",
        lua.create_function(move |_, v: f32| {
            with_vc_mut(&s, |vc| vc.bloom_threshold = v.max(0.0));
            Ok(())
        }),
    )?;
    let s = Rc::clone(scene);
    put(
        table,
        "GetBloomThreshold",
        lua.create_function(move |_, ()| Ok(with_vc(&s, |vc| vc.bloom_threshold).unwrap_or(0.0))),
    )
}

/// Color grading: exposure / contrast / saturation / gamma + tonemap operator.
#[allow(clippy::too_many_lines)] // grandfathered: burn down in #124
fn register_color(lua: &Lua, table: &mlua::Table, scene: &Rc<RefCell<Scene>>) -> Reg {
    let s = Rc::clone(scene);
    put(
        table,
        "SetExposure",
        lua.create_function(move |_, v: f32| {
            with_vc_mut(&s, |vc| vc.exposure = v);
            Ok(())
        }),
    )?;
    let s = Rc::clone(scene);
    put(
        table,
        "GetExposure",
        lua.create_function(move |_, ()| Ok(with_vc(&s, |vc| vc.exposure).unwrap_or(0.0))),
    )?;

    let s = Rc::clone(scene);
    put(
        table,
        "SetContrast",
        lua.create_function(move |_, v: f32| {
            with_vc_mut(&s, |vc| vc.contrast = v);
            Ok(())
        }),
    )?;
    let s = Rc::clone(scene);
    put(
        table,
        "GetContrast",
        lua.create_function(move |_, ()| Ok(with_vc(&s, |vc| vc.contrast).unwrap_or(1.0))),
    )?;

    let s = Rc::clone(scene);
    put(
        table,
        "SetSaturation",
        lua.create_function(move |_, v: f32| {
            with_vc_mut(&s, |vc| vc.saturation = v);
            Ok(())
        }),
    )?;
    let s = Rc::clone(scene);
    put(
        table,
        "GetSaturation",
        lua.create_function(move |_, ()| Ok(with_vc(&s, |vc| vc.saturation).unwrap_or(1.0))),
    )?;

    let s = Rc::clone(scene);
    put(
        table,
        "SetGamma",
        lua.create_function(move |_, v: f32| {
            with_vc_mut(&s, |vc| vc.gamma = v.max(0.01));
            Ok(())
        }),
    )?;
    let s = Rc::clone(scene);
    put(
        table,
        "GetGamma",
        lua.create_function(move |_, ()| Ok(with_vc(&s, |vc| vc.gamma).unwrap_or(2.2))),
    )?;

    let s = Rc::clone(scene);
    put(
        table,
        "SetTonemap",
        lua.create_function(move |_, name: String| {
            if let Some(tm) = parse_tonemap(&name) {
                with_vc_mut(&s, |vc| vc.tonemap = tm);
            }
            Ok(())
        }),
    )?;
    let s = Rc::clone(scene);
    put(
        table,
        "GetTonemap",
        lua.create_function(move |_, ()| {
            Ok(with_vc(&s, |vc| tonemap_name(vc.tonemap).to_string())
                .unwrap_or_else(|| "Aces".to_string()))
        }),
    )
}

/// Screen-space reflections: active + quality string (gated by the High preset).
fn register_ssr(lua: &Lua, table: &mlua::Table, scene: &Rc<RefCell<Scene>>) -> Reg {
    let s = Rc::clone(scene);
    put(
        table,
        "SetSsrActive",
        lua.create_function(move |_, active: bool| {
            with_vc_mut(&s, |vc| vc.ssr_active = active);
            Ok(())
        }),
    )?;
    let s = Rc::clone(scene);
    put(
        table,
        "GetSsrActive",
        lua.create_function(move |_, ()| Ok(with_vc(&s, |vc| vc.ssr_active).unwrap_or(false))),
    )?;

    let s = Rc::clone(scene);
    put(
        table,
        "SetSsrQuality",
        lua.create_function(move |_, q: String| {
            with_vc_mut(&s, |vc| vc.ssr_quality = q.clone());
            Ok(())
        }),
    )?;
    let s = Rc::clone(scene);
    put(
        table,
        "GetSsrQuality",
        lua.create_function(move |_, ()| {
            Ok(with_vc(&s, |vc| vc.ssr_quality.clone()).unwrap_or_default())
        }),
    )
}

/// Camera motion blur: active + sample count over the active camera component.
fn register_motion_blur(lua: &Lua, table: &mlua::Table, scene: &Rc<RefCell<Scene>>) -> Reg {
    let s = Rc::clone(scene);
    put(
        table,
        "SetMotionBlurActive",
        lua.create_function(move |_, active: bool| {
            with_cam_mut(&s, |c| c.motion_blur_active = active);
            Ok(())
        }),
    )?;
    let s = Rc::clone(scene);
    put(
        table,
        "GetMotionBlurActive",
        lua.create_function(move |_, ()| {
            Ok(with_cam(&s, |c| c.motion_blur_active).unwrap_or(false))
        }),
    )?;

    let s = Rc::clone(scene);
    put(
        table,
        "SetMotionBlurSamples",
        lua.create_function(move |_, n: u32| {
            with_cam_mut(&s, |c| c.motion_blur_samples = n.clamp(2, 32));
            Ok(())
        }),
    )?;
    let s = Rc::clone(scene);
    put(
        table,
        "GetMotionBlurSamples",
        lua.create_function(move |_, ()| Ok(with_cam(&s, |c| c.motion_blur_samples).unwrap_or(0))),
    )
}

/// Global quality preset value get/set over the shared resource cell.
fn register_quality(lua: &Lua, table: &mlua::Table, quality: &Rc<RefCell<QualityPreset>>) -> Reg {
    let q = Rc::clone(quality);
    put(
        table,
        "GetQuality",
        lua.create_function(move |_, ()| Ok(quality_name(*q.borrow()).to_string())),
    )?;
    let q = Rc::clone(quality);
    put(
        table,
        "SetQuality",
        lua.create_function(move |_, name: String| {
            if let Some(p) = parse_quality(&name) {
                *q.borrow_mut() = p;
            }
            Ok(())
        }),
    )
}
