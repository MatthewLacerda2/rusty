//! src/api/mod.rs — Engine API facade
//!
//! The single stable API surface shared by Lua scripts, the console REPL and
//! bot-players. Every namespace (`Transform`, `Input`, `Time`, `Physics`,
//! `Scene`, `Random`, `Camera`, `Light`, `Animator`, `Nav`, `Material`,
//! `Assets`, `Texture`, `Shader`, `Sound`, `Particles`, `Layers`, `Graphics`, `Video`,
//! `Storage`, `Canvas`, `RectTransform`, `UI`, `Image`, `CanvasGroup`, `RectMask`,
//! `Application`, plus the dev-only `Debug`)
//! is registered from this tree onto the live Lua runtime.
//! `scripting`
//! owns the runtime and lifecycle; `api` owns the surface. One surface, three
//! callers — they never drift apart.
//!
//! Allowed deps: core, components, physics, navigation, render, time, scripting,
//! asset (for the `Assets` manifest).

pub mod animator;
pub mod application;
pub mod assets;
pub mod audio;
pub mod camera;
pub mod canvas;
pub mod canvas_group;
#[cfg(feature = "dev")]
pub mod debug;
pub mod decals;
pub mod graphics;
pub mod image;
pub mod input;
pub mod layers;
pub mod light;
pub mod lighting;
pub(crate) mod lua_json;
pub mod material;
pub mod nav;
pub mod particle;
pub mod physics;
pub mod probe;
pub mod random;
pub mod rect_mask;
pub mod rect_transform;
pub mod reflection;
pub mod scene;
pub mod scene_prefab;
pub mod shader;
pub mod snapshot;
mod snapshot_components;
pub mod sound;
pub mod storage;
pub mod texture;
pub mod time;
pub mod transform;
pub mod ui;
pub mod video;

use std::cell::RefCell;

use mlua::{Function, Lua, Table};

use crate::audio::AudioMaestro;
use crate::core::application::Application;
use crate::core::input::InputState;
use crate::core::quality::QualityPreset;
use crate::core::random::Random;
use crate::core::storage::Storage;
use crate::core::video::VideoSettings;
use crate::navigation::NavigationGraph;
use crate::physics::PhysicsWorld;
use crate::scene::Camera;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;
use crate::time::Time;
use crate::ui::ScreenSize;

/// Result alias every namespace registrar returns.
pub type Reg = Result<(), String>;

/// Scoped engine-resource references bundled for a single script evaluation.
///
/// Every field is a plain borrow of the engine's `RefCell<T>` — no `Rc` clone
/// occurs. The lifetime `'scope` ties these references to the `mlua::Scope`
/// that created the closures, so the Lua functions cannot outlive the call-frame
/// that holds the borrows.
pub struct ApiScopedCtx<'scope> {
    pub scene: &'scope RefCell<Scene>,
    pub input: &'scope RefCell<InputState>,
    pub nav: &'scope RefCell<NavigationGraph>,
    pub camera: &'scope RefCell<Camera>,
    pub time: &'scope RefCell<Time>,
    pub physics: &'scope RefCell<Option<PhysicsWorld>>,
    pub console: &'scope RefCell<ConsoleLogs>,
    pub storage: &'scope RefCell<Storage>,
    /// Global post-FX scalability tier, shared with the platform layer so a
    /// `Graphics.SetQuality` write reaches `renderer.set_quality`.
    pub quality: &'scope RefCell<QualityPreset>,
    /// Runtime video settings (resolution / vsync / fullscreen), shared with the
    /// platform layer so a `Video.*` write reaches the surface + window.
    pub video: &'scope RefCell<VideoSettings>,
    /// The current scene file — `Scene.Save()`'s no-path write-back target. Shared
    /// with the platform layer (synced from `EditorUi.current_scene_path`).
    pub scene_path: &'scope RefCell<Option<String>>,
    /// Whether the world is in play mode — mirrored from `GameWorld` so the
    /// dev-only `Debug.Snapshot` can report play-state without reaching into the
    /// `Resources` the evaluator deliberately doesn't hold.
    pub is_playing: &'scope RefCell<bool>,
    /// The audio engine singleton (#212): the `Audio` namespace drives it
    /// (Play/Stop/SetVolume/PlayAt + master volume). The same maestro the play-mode
    /// systems use, so a scripted play and `play_on_start` share one device + log.
    pub audio: &'scope RefCell<AudioMaestro>,
    /// The seeded gameplay RNG (#443): the `Random` namespace and the sandboxed
    /// `math.random` both draw from it.
    pub random: &'scope RefCell<Random>,
    /// The screen the UI lays out on (#417); resolved against `video` when no
    /// platform reported a game-view size (headless).
    pub screen: &'scope RefCell<ScreenSize>,
    /// Build settings + the quit request (#431): `Application.Quit` raises the flag
    /// the host (player / editor / harness) reads after the tick.
    pub application: &'scope RefCell<Application>,
}

/// Register every namespace onto `lua` using `scope`-tied closures that borrow
/// the engine resources via `ctx`. This is the one place the whole script
/// surface is wired up, shared by gameplay scripts, the console REPL and
/// bot-players.
pub fn register<'lua, 'scope>(
    lua: &'lua Lua,
    scope: &mlua::Scope<'lua, 'scope>,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    transform::register(lua, scope, ctx.scene)?;
    // `Assets` borrows no engine state (it walks the project asset root on each
    // call), so it stays a plain static registrar even under the scoped surface.
    assets::register(lua)?;
    // `Texture` (#270) also borrows no engine state — it reads a recipe and writes a
    // PNG file — so it likewise registers as a plain static namespace.
    texture::register(lua)?;
    // `Shader` (#272) likewise borrows no engine state — it reads a recipe, validates
    // it through naga_oil, and writes a `.wgsl` file — a plain static namespace.
    shader::register(lua)?;
    // `Sound` (#357) is the same story for audio: it reads a patch and writes a
    // `.wav`, so it too registers as a plain static namespace.
    sound::register(lua)?;
    material::register(lua, scope, ctx.scene)?;
    animator::register(lua, scope, ctx.scene, ctx.console)?;
    input::register_readable(lua, scope, ctx.input)?;
    scene::register(lua, scope, ctx.scene, ctx.scene_path, ctx.is_playing)?;
    nav::register(lua, scope, ctx.scene, ctx.nav)?;
    physics::register(lua, scope, ctx.scene)?;
    physics::register_hitscan(lua, scope, ctx.scene, ctx.physics)?;
    time::register(lua, scope, ctx.time)?;
    random::register(lua, scope, ctx.random)?;
    camera::register(lua, scope, ctx.camera)?;
    light::register(lua, scope, ctx.scene)?;
    probe::register(lua, scope, ctx.scene)?;
    reflection::register(lua, scope, ctx.scene, ctx.scene_path)?;
    lighting::register(lua, scope, ctx.scene, ctx.scene_path, ctx.nav)?;
    particle::register(lua, scope, ctx.scene)?;
    audio::register(lua, scope, ctx.scene, ctx.audio, ctx.time, ctx.camera)?;
    canvas::register(lua, scope, ctx.scene, ctx.screen, ctx.video)?;
    rect_transform::register(lua, scope, ctx.scene)?;
    ui::register(lua, scope, ctx.scene, ctx.screen, ctx.video)?;
    image::register(lua, scope, ctx.scene)?;
    canvas_group::register(lua, scope, ctx.scene)?;
    rect_mask::register(lua, scope, ctx.scene)?;
    decals::register(lua, scope, ctx.scene)?;
    layers::register(lua, scope, ctx.scene)?;
    graphics::register(lua, scope, ctx.scene, ctx.quality)?;
    video::register(lua, scope, ctx.video)?;
    storage::register(lua, scope, ctx.storage)?;
    input::register_writable(lua, scope, ctx.input)?;
    application::register(lua, scope, ctx.application)?;
    #[cfg(feature = "dev")]
    debug::register(lua, scope, ctx)?;
    Ok(())
}

/// Set a named entry on `table` from a built function value.
pub(crate) fn put(table: &Table, name: &str, f: mlua::Result<Function>) -> Reg {
    table
        .set(name, f.map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

/// Fetch an existing global table so we can extend it in place.
pub(crate) fn global_table<'lua>(lua: &'lua Lua, name: &str) -> Result<Table<'lua>, String> {
    lua.globals().get(name).map_err(|e| e.to_string())
}
