//! src/api/mod.rs — Engine API facade
//!
//! The single stable API surface shared by Lua scripts, the console REPL and
//! bot-players. Every namespace (`Transform`, `Input`, `Time`, `Physics`,
//! `Scene`, `Health`, `Camera`, `Animator`, `Nav`, `Material`, `Particles`,
//! `Layers`, `Storage`, plus the dev-only `Debug`) is registered from this tree
//! onto the live Lua runtime.
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
pub mod health;
pub mod input;
pub mod layers;
pub mod material;
pub mod nav;
pub mod particle;
pub mod physics;
pub mod scene;
pub mod storage;
pub mod time;
pub mod transform;

use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Function, Lua, Table};

use crate::core::input::InputState;
use crate::core::storage::Storage;
use crate::navigation::NavigationGraph;
use crate::physics::PhysicsWorld;
use crate::render::Camera;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;
use crate::time::Time;

/// Result alias every namespace registrar returns.
pub type Reg = Result<(), String>;

/// The shared engine-resource handles every namespace binds against. Bundled
/// into one struct so each registrar takes a single context. The live `physics`
/// handle is what lets `Physics.Raycast`/`Shoot` reach the same rapier world the
/// engine hitscan uses; it is `None` until Play builds the world.
pub struct ApiCtx {
    pub scene: Rc<RefCell<Scene>>,
    pub input: Rc<RefCell<InputState>>,
    pub nav: Rc<RefCell<NavigationGraph>>,
    pub camera: Rc<RefCell<Camera>>,
    pub time: Rc<RefCell<Time>>,
    pub physics: Rc<RefCell<Option<PhysicsWorld>>>,
    pub console: Rc<RefCell<ConsoleLogs>>,
    pub storage: Rc<RefCell<Storage>>,
}

/// Register every namespace onto `lua`. This is the one place the whole script
/// surface is wired up, shared by gameplay scripts, the console REPL and
/// bot-players.
pub fn register(lua: &Lua, ctx: &ApiCtx) -> Reg {
    transform::register(lua, &ctx.scene)?;
    material::register(lua, &ctx.scene)?;
    animator::register(lua, &ctx.scene, &ctx.console)?;
    input::register_readable(lua, &ctx.input)?;
    scene::register(lua, &ctx.scene, &ctx.console)?;
    nav::register(lua, &ctx.scene, &ctx.nav)?;
    physics::register(lua, &ctx.scene)?;
    health::register(lua, &ctx.scene, &ctx.console)?;
    physics::register_hitscan(lua, &ctx.scene, &ctx.physics, &ctx.console)?;
    time::register(lua, &ctx.time)?;
    camera::register(lua, &ctx.camera)?;
    particle::register(lua, &ctx.scene)?;
    decals::register(lua, &ctx.scene)?;
    layers::register(lua, &ctx.scene)?;
    storage::register(lua, &ctx.storage)?;
    input::register_writable(lua, &ctx.input)?;
    #[cfg(feature = "dev")]
    debug::register(lua, &ctx.console)?;
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
