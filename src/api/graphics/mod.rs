//! src/api/graphics.rs — `Graphics` namespace.
//!
//! Live get/set over the engine's EXISTING post-FX and quality knobs, so a script
//! can drive its own settings logic in Lua. Two backing stores:
//!
//! * The post effects (`effects`, #1467): one registrar per effect, over the main
//!   camera's active `PostProcessingComponent` or, for motion blur and FXAA, its
//!   `CameraComponent`. The main camera is the screen stack's base
//!   (`scene::camera::main_camera`, Unity's `Camera.main`, #1434), the camera whose
//!   components the renderer packs into the screen's post-FX EVERY frame, so a write
//!   here takes effect next frame for free — no GPU pipeline is touched from script.
//! * The same volume's lighting settings: the sun's shadow cascades (`shadows`,
//!   #435), which the shadow pass re-fits from every frame, and its ambient
//!   occlusion (`ssao`, #436).
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

pub(crate) mod effects;
mod fog;
mod options;
mod shadows;
mod ssao;
mod state;

use mlua::Lua;

use self::state::register_quality;
use super::Reg;
use crate::core::application::Application;
use crate::core::settings::GraphicsSettings;
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

    effects::register(scope, &table, scene)?;
    register_quality(scope, &table, graphics)?;
    options::register_options(scope, &table, graphics, app)?;
    shadows::register_shadows(scope, &table, scene)?;
    fog::register_fog(scope, &table, scene)?;
    ssao::register_ssao(scope, &table, scene)?;

    lua.globals()
        .set("Graphics", table)
        .map_err(|e| e.to_string())
}
