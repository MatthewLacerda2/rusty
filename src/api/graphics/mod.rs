//! src/api/graphics/mod.rs — `Graphics` namespace.
//!
//! Live get/set over the engine's EXISTING post-FX and quality knobs, so a script
//! can drive its own settings logic in Lua. What it reaches:
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
//! * The project's quality presets (`quality`, #1522) on `Application`, and the
//!   running one's values in the shared graphics cell, which the platform layer
//!   hands to the renderer each frame. No preset gates an effect: each effect's
//!   quality level reads its values from the running preset. The scene's render
//!   scale (#1498) is a value in the same cell (`render_scale`): the player's choice
//!   over the project's default on `Application` (#1715), resolved on the display
//!   scale the `ScreenSize` resource holds.
//! * The player's graphics options (`options`, #1438) in the same cell, which gate
//!   every camera's effects, and the project's defaults for them on `Application`.
//!
//! Every write is ONE-WAY into render-only state: it never feeds back into
//! `FixedUpdate`, so toggling these knobs can't change the deterministic sim. The
//! per-volume setters route through the shared `authoring::{rendering::camera,
//! post_processing}` ops the editor cards use, so panel and binding never drift.

pub(crate) mod effects;
mod fog;
mod options;
mod quality;
mod render_scale;
mod shadows;
mod ssao;
mod state;

use mlua::Lua;

use super::Reg;
use crate::api::ApiCell;
use crate::core::application::Application;
use crate::core::settings::GraphicsSettings;
use crate::scene::Scene;
use crate::ui::ScreenSize;

/// Register the `Graphics` namespace onto `lua`.
pub fn register(
    lua: &Lua,
    scene: ApiCell<Scene>,
    (graphics, screen): (ApiCell<GraphicsSettings>, ApiCell<ScreenSize>),
    app: ApiCell<Application>,
) -> Reg {
    let table = lua.create_table().map_err(|e| e.to_string())?;

    effects::register(lua, &table, scene)?;
    quality::register_quality(lua, &table, graphics, app)?;
    render_scale::register(lua, &table, (graphics, screen), app)?;
    options::register_options(lua, &table, graphics, app)?;
    shadows::register_shadows(lua, &table, scene)?;
    fog::register_fog(lua, &table, scene)?;
    ssao::register_ssao(lua, &table, scene)?;

    lua.globals()
        .set("Graphics", table)
        .map_err(|e| e.to_string())
}
