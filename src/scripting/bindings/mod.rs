//! src/scripting/bindings/ — issue #5 Lua namespaces.
//!
//! The stable script API surface mirrored by `src/api/*`: `Health`, `Time`,
//! `Camera`, the writable half of `Input`, and `Physics.Raycast` / `Physics.Shoot`
//! (hitscan lifted out of `app/play.rs`). `Debug.*` is registered only under the
//! `dev` feature, mirroring Unity's `[Conditional]` `Debug` — stripped from ship
//! builds.
//!
//! Split into `combat` (Health + Physics hitscan) and `world` (Time, Camera,
//! writable Input, Debug) so each file stays under the size cap. Every helper
//! attaches one namespace, keeping `init_runtime` a flat list of calls.

mod combat;
mod world;

use std::cell::RefCell;
use std::rc::Rc;

use mlua::{Function, Lua, Table};

use crate::core::input::InputState;
use crate::core::scene::Scene;
use crate::render::Camera;
use crate::scripting::ConsoleLogs;
use crate::time::Time;

type Reg = Result<(), String>;

/// Register every issue-#5 namespace onto `lua`.
pub fn register(
    lua: &Lua,
    scene: &Rc<RefCell<Scene>>,
    input: &Rc<RefCell<InputState>>,
    camera: &Rc<RefCell<Camera>>,
    time: &Rc<RefCell<Time>>,
    console: &Rc<RefCell<ConsoleLogs>>,
) -> Reg {
    combat::register_health(lua, scene, console)?;
    combat::register_physics_hitscan(lua, scene, console)?;
    world::register_time(lua, time)?;
    world::register_camera(lua, camera)?;
    world::register_input_writable(lua, input)?;
    #[cfg(feature = "dev")]
    world::register_debug(lua, console)?;
    Ok(())
}

/// Set a named entry on `table` from a built function value.
fn put(table: &Table, name: &str, f: mlua::Result<Function>) -> Reg {
    table
        .set(name, f.map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

/// Fetch an existing global table so we can extend it in place.
fn global_table<'lua>(lua: &'lua Lua, name: &str) -> Result<Table<'lua>, String> {
    lua.globals().get(name).map_err(|e| e.to_string())
}
