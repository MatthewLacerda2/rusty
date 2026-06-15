//! src/api/mod.rs — Engine API facade
//!
//! The single stable API surface shared by Lua scripts, the console REPL and
//! bot-players. Every namespace (`Transform`, `Input`, `Time`, `Physics`,
//! `Scene`, `Health`, `Camera`, `Animator`, `Nav`, `Material`, plus the dev-only
//! `Debug`) is registered from this tree onto the live Lua runtime. `scripting`
//! owns the runtime and lifecycle; `api` owns the surface. One surface, three
//! callers — they never drift apart.
//!
//! The bindings no longer capture `Rc<RefCell>` clones with a `'static` lifetime
//! (#57). They are SCOPED callbacks (`scope.create_function`) registered per-run
//! inside `lua.scope`, capturing borrowed `&RefCell<&mut T>` references that point
//! straight at the owned engine state the systems hold. The transient `RefCell`s
//! are stack locals at the script-run boundary, NOT fields of `Resources`/`World`:
//! they let several re-entrant bindings share `&mut Scene` &c. for the scope's
//! duration without any heap-shared interior mutability.
//!
//! Allowed deps: core, components, physics, navigation, render, time, scripting.

pub mod animator;
pub mod camera;
#[cfg(feature = "dev")]
pub mod debug;
pub mod health;
pub mod input;
pub mod material;
pub mod nav;
pub mod physics;
pub mod scene;
pub mod time;
pub mod transform;

use std::cell::RefCell;

use mlua::{Function, Lua, Scope, Table};

use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::physics::PhysicsWorld;
use crate::render::Camera;
use crate::scene::Scene;
use crate::scripting::ConsoleLogs;
use crate::time::Time;

/// Result alias every namespace registrar returns.
pub type Reg = Result<(), String>;

/// The shared engine state every namespace binds against, each borrowed through a
/// transient `RefCell` that lives for the script-run scope. Bundled into one
/// struct so each registrar takes a single context. The live `physics` cell is
/// what lets `Physics.Raycast`/`Shoot` reach the same rapier world the engine
/// hitscan uses; the inner `Option` is `None` until Play builds the world.
pub struct ApiCtx<'a> {
    pub scene: &'a RefCell<&'a mut Scene>,
    pub input: &'a RefCell<&'a mut InputState>,
    pub nav: &'a RefCell<&'a mut NavigationGraph>,
    pub camera: &'a RefCell<&'a mut Camera>,
    pub time: &'a RefCell<&'a mut Time>,
    pub physics: &'a RefCell<&'a mut Option<PhysicsWorld>>,
    pub console: &'a RefCell<&'a mut ConsoleLogs>,
}

/// Register every namespace onto `lua` via scoped callbacks. This is the one place
/// the whole script surface is wired up, shared by gameplay scripts, the console
/// REPL and bot-players. Called once per run, inside `lua.scope`, so the bindings
/// borrow the live engine state for exactly the scope's duration.
pub fn register<'a, 'scope>(
    scope: &Scope<'a, 'scope>,
    lua: &'a Lua,
    ctx: &'scope ApiCtx<'scope>,
) -> Reg {
    transform::register(scope, lua, ctx.scene)?;
    material::register(scope, lua, ctx.scene)?;
    animator::register(scope, lua, ctx.scene, ctx.console)?;
    input::register_readable(scope, lua, ctx.input)?;
    scene::register(scope, lua, ctx.scene, ctx.console)?;
    nav::register(scope, lua, ctx.scene, ctx.nav)?;
    physics::register(scope, lua, ctx.scene)?;
    health::register(scope, lua, ctx.scene, ctx.console)?;
    physics::register_hitscan(scope, lua, ctx.scene, ctx.physics, ctx.console)?;
    time::register(scope, lua, ctx.time)?;
    camera::register(scope, lua, ctx.camera)?;
    input::register_writable(scope, lua, ctx.input)?;
    #[cfg(feature = "dev")]
    debug::register(scope, lua, ctx.console)?;
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
