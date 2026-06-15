use mlua::{Lua, RegistryKey, Table};
use std::collections::HashMap;
use std::path::Path;

use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::physics::PhysicsWorld;
use crate::render::Camera;
use crate::scene::Scene;
use crate::time::Time;

use super::console::ConsoleLogs;

/// The live engine state a script run borrows. Bundled so the run methods
/// (`start_scripts`/`update_scripts`/`dispatch_trigger_events`/`eval`) take a
/// single argument. Every field is a plain `&mut` to the owned engine state the
/// systems hold (#57) — no `Rc`, no `RefCell`. The run method wraps each one in a
/// transient stack `RefCell` and opens ONE `lua.scope` around the lifecycle calls,
/// so the bindings reach the very same values without heap-shared mutability.
pub struct ScriptCtx<'a> {
    pub scene: &'a mut Scene,
    pub input: &'a mut InputState,
    pub nav: &'a mut NavigationGraph,
    pub console: &'a mut ConsoleLogs,
    pub camera: &'a mut Camera,
    pub time: &'a mut Time,
    pub physics: &'a mut Option<PhysicsWorld>,
}

/// The Lua runtime. Holds ONLY the `Lua` state and the per-entity script tables
/// cached in the Lua registry; it no longer owns any engine-state handles (#57).
/// The bindings are registered per-run inside `lua.scope` over borrowed engine
/// state (see [`ScriptCtx`]), not once with a `'static` lifetime.
pub struct ScriptManager {
    pub(super) lua: Option<Lua>,
    pub(super) entity_scripts: HashMap<u32, RegistryKey>,
}

impl Default for ScriptManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ScriptManager {
    pub fn new() -> Self {
        Self {
            lua: None,
            entity_scripts: HashMap::new(),
        }
    }

    /// Create a fresh Lua environment and clear the cached script tables. The API
    /// surface is NOT registered here — it is wired per-run inside a `lua.scope`
    /// over borrowed engine state (see [`ScriptCtx`]), so the bindings capture
    /// non-`'static` references to the live owned data (#57).
    pub fn init_runtime(&mut self) -> Result<(), String> {
        let lua = Lua::new();
        self.lua = Some(lua);
        self.entity_scripts.clear();
        Ok(())
    }

    /// Loads and runs a script file, then registers its lifecycle methods
    pub fn load_entity_script(&mut self, entity_id: u32, script_path: &str) -> Result<(), String> {
        let lua = self.lua.as_ref().ok_or("Lua runtime not initialized")?;

        if !Path::new(script_path).exists() {
            return Err(format!("Script file not found: {}", script_path));
        }

        let script_code = std::fs::read_to_string(script_path)
            .map_err(|e| format!("Failed to read script: {}", e))?;

        // Load the script chunk
        let chunk = lua.load(&script_code);
        let table: Table = chunk
            .eval()
            .map_err(|e| format!("Syntax error compiling {}: {}", script_path, e))?;

        // Cache the returned lifecycle table in the Lua registry
        let reg_key = lua
            .create_registry_value(table)
            .map_err(|e| format!("Failed to register script table: {}", e))?;

        self.entity_scripts.insert(entity_id, reg_key);

        Ok(())
    }

    /// Stops and clears the script environment
    pub fn shutdown(&mut self) {
        self.entity_scripts.clear();
        self.lua = None;
    }

    /// Run a Lua chunk against the live runtime, registering the API over `ctx`
    /// for the call. Test-only entry point for the issue-#5 API coverage; the
    /// windowed/headless paths drive scripts through the lifecycle callbacks.
    #[cfg(test)]
    pub(super) fn exec(&self, ctx: &mut ScriptCtx, code: &str) -> Result<(), String> {
        let lua = self.lua.as_ref().ok_or("runtime not initialized")?;
        super::lifecycle::with_api_scope(lua, ctx, |lua| {
            lua.load(code).exec().map_err(|e| e.to_string())
        })
    }

    /// Whether the live runtime exists yet (only during play / a loaded scenario).
    pub fn is_live(&self) -> bool {
        self.lua.is_some()
    }
}
