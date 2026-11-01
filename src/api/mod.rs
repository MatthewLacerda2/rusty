//! src/api/mod.rs — Engine API facade
//!
//! The single stable API surface shared by Lua scripts, the console REPL and
//! bot-players. Every namespace (one page each in `docs/api/`, which
//! `tests/api_doc_drift.rs` holds to the live surface) is registered from this tree
//! onto the live Lua runtime. Layers above the sim add their own through [`extend`]:
//! `dev` installs `Debug`, `Editor`, the provider verbs and the GPU bake verbs that
//! way (#737, `dev::lua_surface`), so the sim surface never imports the platform layer.
//! `scripting` owns the runtime and lifecycle; `api` owns the surface. One surface, three
//! callers — they never drift apart.

// Panic-free sim (#195, #962): bare `.unwrap()` is denied in every sim module (use `?`
// or a documented `.expect(...)`); test code is exempt via clippy.toml. See docs/linting.md.
#![deny(clippy::unwrap_used)]

// lint: allow(dir-size) a flat registry: one file per Lua namespace, so the list is the surface's index
// sorted: namespace modules and their helpers; no module reads another's declaration order
pub mod animator;
pub mod application;
pub mod assets;
pub mod audio;
pub mod backdrop_filter;
pub mod camera;
pub mod canvas;
pub mod canvas_group;
pub mod character_controller;
pub mod component;
pub mod decals;
pub mod entity_arg;
pub mod graphics;
pub mod image;
pub mod input;
pub mod joint;
pub mod layers;
pub mod layout_element;
pub mod layout_group;
pub mod light;
pub mod light_probe_proxy_volume;
pub mod lighting;
pub mod line;
pub mod lod_group;
pub(crate) mod lua_json;
pub mod mask;
pub mod material;
pub mod nav;
pub mod nav_modifier;
pub mod nav_obstacle;
pub mod offmesh_link;
pub mod particle;
pub mod physics;
pub mod probe;
pub mod ragdoll;
pub mod random;
pub mod rect_mask;
pub mod rect_transform;
pub mod reflection;
pub mod reverb_zone;
mod ribbon_style;
pub mod scene;
pub mod scene_hierarchy;
pub mod scene_load;
pub mod scene_prefab;
pub mod selectable;
pub mod shader;
pub mod shape;
pub mod snapshot;
pub mod sound;
pub mod storage;
pub mod text;
mod text_effects;
mod text_layout;
pub mod texture;
pub mod time;
pub mod timer;
pub mod trail;
pub mod transform;
pub mod tween;
pub mod ui;
mod ui_look;
pub mod video;

pub use lua_json::recipe_from_json_str;

use std::cell::RefCell;
use std::sync::RwLock;

use mlua::{Function, Lua, Table};

use crate::audio::AudioMaestro;
use crate::core::application::Application;
use crate::core::frame_stats::FrameStats;
use crate::core::input::InputState;
use crate::core::random::Random;
use crate::core::settings::video::VideoSettings;
use crate::core::settings::GraphicsSettings;
use crate::core::storage::Storage;
use crate::navigation::NavigationGraph;
use crate::physics::PhysicsWorld;
use crate::scene::Camera;
use crate::scene::Scene;
use crate::scripting::{ConsoleLogs, TimerScheduler};
use crate::time::Time;
use crate::ui::{EventSystem, ScreenSize};

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
    /// Post-FX tier + player options, shared with the platform layer so a
    /// `Graphics.SetQuality`/`SetOption` write reaches the renderer (#1438).
    pub graphics: &'scope RefCell<GraphicsSettings>,
    /// Runtime video settings (resolution / vsync / fullscreen), shared with the
    /// platform layer so a `Video.*` write reaches the surface + window.
    pub video: &'scope RefCell<VideoSettings>,
    /// The current scene file — `Scene.Save()`'s no-path write-back target. Shared
    /// with the platform layer (the editor's current scene file is this cell, #1366).
    pub scene_path: &'scope RefCell<Option<String>>,
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
    /// Frame stats (#433): filled by the dev layer's schedule probe and capture
    /// path, read by the dev-only `Debug.Stats()`. Never read by the sim.
    pub stats: &'scope RefCell<FrameStats>,
    /// Pending script timers and coroutines (#444), behind the `Timer` namespace.
    pub timers: &'scope RefCell<TimerScheduler>,
    /// The UI event system (#420): selection, hover and press state behind the `UI`
    /// namespace's pointer and focus verbs.
    pub event_system: &'scope RefCell<EventSystem>,
}

/// The navigation namespaces: `Navigation` + `NavMeshAgent`, `NavMeshObstacle`,
/// `NavMeshModifierVolume` (#460) and `OffMeshLink`.
fn register_navigation<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    // sorted: each registrar makes its own global table
    nav::register(lua, scope, ctx.scene, ctx.nav)?;
    nav_modifier::register(lua, scope, ctx.scene)?;
    nav_obstacle::register(lua, scope, ctx.scene)?;
    offmesh_link::register(lua, scope, ctx.scene, ctx.nav)
}

/// The scene namespaces: `Scene` (structure) and `Component` (fields by name, #1369).
fn register_scene<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    // sorted: each registrar makes its own global table
    component::register(lua, scope, ctx.scene)?;
    scene::register(lua, scope, ctx.scene, ctx.scene_path)
}

/// Register every namespace onto `lua` using `scope`-tied closures that borrow
/// the engine resources via `ctx`. This is the one place the whole script
/// surface is wired up, shared by gameplay scripts, the console REPL and
/// bot-players.
pub fn register<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    ctx: &ApiScopedCtx<'scope>,
) -> Reg {
    // sorted: each registrar makes its own global table; the ones extending a table
    // (`register_hitscan`, `register_scoped`, `register_events`, `register_writable`)
    // sort after the one that made it
    animator::register(lua, scope, ctx.scene, ctx.console)?;
    application::register(lua, scope, ctx.application)?;
    // `Assets` mostly borrows no engine state (it walks the project asset root on
    // each call), so it stays a plain static registrar; only the import-setting
    // setter that re-imports the scene's meshes (#831) is scoped.
    assets::register(lua)?;
    assets::register_scoped(lua, scope, ctx.scene)?;
    audio::register(lua, scope, ctx.scene, ctx.audio, ctx.time, ctx.camera)?;
    camera::register(lua, scope, ctx)?;
    canvas::register(lua, scope, ctx.scene, ctx.screen, ctx.video)?;
    character_controller::register(lua, scope, ctx.scene, ctx.physics)?;
    decals::register(lua, scope, ctx.scene)?;
    graphics::register(lua, scope, ctx.scene, ctx.graphics, ctx.application)?;
    input::register_readable(lua, scope, ctx.input)?;
    input::register_writable(lua, scope, ctx.input)?;
    joint::register(lua, scope, ctx.scene)?;
    layers::register(lua, scope, ctx.scene)?;
    light::register(lua, scope, ctx.scene)?;
    light_probe_proxy_volume::register(lua, scope, ctx.scene)?;
    lighting::register(lua, scope, ctx.scene)?;
    line::register(lua, scope, ctx.scene)?;
    lod_group::register(lua, scope, ctx.scene)?;
    material::register(lua, scope, ctx.scene)?;
    particle::register(lua, scope, ctx.scene)?;
    physics::register(lua, scope, ctx.scene)?;
    physics::register_hitscan(lua, scope, ctx.scene, ctx.physics)?;
    probe::register(lua, scope, ctx.scene)?;
    ragdoll::register(lua, scope, ctx.scene, ctx.physics)?;
    random::register(lua, scope, ctx.random)?;
    rect_transform::register(lua, scope, ctx.scene, (ctx.screen, ctx.video))?;
    reflection::register(lua, scope, ctx.scene)?;
    register_navigation(lua, scope, ctx)?;
    register_scene(lua, scope, ctx)?;
    register_ui_components(lua, scope, ctx.scene)?;
    reverb_zone::register(lua, scope, ctx.scene)?;
    selectable::register(lua, scope, ctx.scene, ctx.event_system)?;
    // `Shader` (#272) borrows no engine state — it reads a recipe, validates it
    // through naga_oil, and writes a `.wgsl` file — so it is a plain static namespace.
    shader::register(lua)?;
    shape::register(lua, scope, ctx.scene)?;
    // `Sound` (#357) is `Shader`'s story for audio: it reads a patch and writes a
    // `.wav`, so it too registers as a plain static namespace.
    sound::register(lua)?;
    storage::register(lua, scope, ctx.storage)?;
    text::register(lua, scope, ctx.scene, ctx.screen, ctx.video)?;
    // `Texture` (#270) borrows no engine state either — it reads a recipe and writes
    // a PNG file — so it too registers as a plain static namespace.
    texture::register(lua)?;
    time::register(lua, scope, ctx.time)?;
    timer::register(lua, scope, ctx.scene, ctx.timers, ctx.console)?;
    trail::register(lua, scope, ctx.scene)?;
    transform::register(lua, scope, ctx.scene)?;
    tween::register(lua, scope, ctx.scene, ctx.timers)?;
    ui::register(lua, scope, ctx.scene, (ctx.screen, ctx.video), ctx.camera)?;
    ui::register_events(lua, scope, ctx)?;
    video::register(lua, scope, ctx.video)?;

    // Last, so an extension can add verbs to a namespace registered above.
    for ext in EXTENSIONS.read().map_err(|e| e.to_string())?.iter() {
        ext(lua, scope, ctx)?;
    }
    Ok(())
}

/// A registrar a layer above the sim adds to the surface: the same shape as
/// [`register`], run after every built-in namespace on every scope.
pub type Extension =
    for<'scope> fn(&Lua, &'scope mlua::Scope<'scope, '_>, &ApiScopedCtx<'scope>) -> Reg;

static EXTENSIONS: RwLock<Vec<Extension>> = RwLock::new(Vec::new());

/// Add `ext` to the surface every later [`register`] call installs (#737). This is how
/// the platform layer contributes bindings without the sim importing it: the arrow
/// runs `dev → api`. Process-wide; callers install once (see `dev::install_api`).
pub fn extend(ext: Extension) {
    if let Ok(mut exts) = EXTENSIONS.write() {
        exts.push(ext);
    }
}

/// The UI component namespaces that borrow only the scene: the graphic, its
/// group, clips and backdrop (#418, #426, #428), and the layout pair (#421).
fn register_ui_components<'scope>(
    lua: &Lua,
    scope: &'scope mlua::Scope<'scope, '_>,
    scene: &'scope RefCell<Scene>,
) -> Reg {
    // sorted: each registrar makes its own global table
    backdrop_filter::register(lua, scope, scene)?;
    canvas_group::register(lua, scope, scene)?;
    image::register(lua, scope, scene)?;
    layout_element::register(lua, scope, scene)?;
    layout_group::register(lua, scope, scene)?;
    mask::register(lua, scope, scene)?;
    rect_mask::register(lua, scope, scene)
}

/// Set a named entry on `table` from a built function value.
pub(crate) fn put(table: &Table, name: &str, f: mlua::Result<Function>) -> Reg {
    table
        .set(name, f.map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

/// Fetch an existing global table so we can extend it in place.
pub(crate) fn global_table(lua: &Lua, name: &str) -> Result<Table, String> {
    lua.globals().get(name).map_err(|e| e.to_string())
}
