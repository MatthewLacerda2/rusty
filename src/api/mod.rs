//! src/api/mod.rs — Engine API facade
//!
//! The single stable API surface shared by Lua scripts, the console REPL and
//! bot-players. Every namespace (`Transform`, `Input`, `Time`, `Physics`,
//! `Scene`, `Health`, `Camera`, `Light`, `Animator`, `Nav`, `Material`,
//! `Particles`, `Layers`, `Graphics`, `Video`, `Storage`, plus the dev-only `Debug`)
//! is registered from this tree onto the live Lua runtime.
//! `scripting`
//! owns the runtime and lifecycle; `api` owns the surface. One surface, three
//! callers — they never drift apart.
//!
//! Allowed deps: core, components, physics, navigation, render, time, scripting.

pub mod animator;
pub mod camera;
#[cfg(feature = "dev")]
pub mod debug;
pub mod decals;
pub mod graphics;
pub mod health;
pub mod input;
pub mod layers;
pub mod light;
pub mod material;
pub mod nav;
pub mod particle;
pub mod physics;
pub mod scene;
pub mod storage;
pub mod time;
pub mod transform;
pub mod video;

use std::cell::RefCell;

use mlua::{Function, Lua, Table};

use crate::core::input::InputState;
use crate::core::storage::Storage;
use crate::core::video::VideoSettings;
use crate::navigation::NavigationGraph;
use crate::physics::PhysicsWorld;
use crate::render::postfx::QualityPreset;
use crate::render::Camera;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;
use crate::time::Time;

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
    material::register(lua, scope, ctx.scene)?;
    animator::register(lua, scope, ctx.scene, ctx.console)?;
    input::register_readable(lua, scope, ctx.input)?;
    scene::register(lua, scope, ctx.scene, ctx.scene_path)?;
    nav::register(lua, scope, ctx.scene, ctx.nav)?;
    physics::register(lua, scope, ctx.scene)?;
    health::register(lua, scope, ctx.scene, ctx.console)?;
    physics::register_hitscan(lua, scope, ctx.scene, ctx.physics, ctx.console)?;
    time::register(lua, scope, ctx.time)?;
    camera::register(lua, scope, ctx.camera)?;
    light::register(lua, scope, ctx.scene)?;
    particle::register(lua, scope, ctx.scene)?;
    decals::register(lua, scope, ctx.scene)?;
    layers::register(lua, scope, ctx.scene)?;
    graphics::register(lua, scope, ctx.scene, ctx.quality)?;
    video::register(lua, scope, ctx.video)?;
    storage::register(lua, scope, ctx.storage)?;
    input::register_writable(lua, scope, ctx.input)?;
    #[cfg(feature = "dev")]
    debug::register(lua, scope, ctx.console)?;
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
