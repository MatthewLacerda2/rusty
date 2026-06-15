use mlua::{Lua, RegistryKey, Table};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use crate::api::{self, ApiCtx};
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::render::Camera;
use crate::scene::Scene;
use crate::time::Time;

use super::console::ConsoleLogs;

pub struct ScriptManager {
    pub(super) lua: Option<Lua>,
    pub(super) entity_scripts: HashMap<u32, RegistryKey>,
    pub(super) scene: Rc<RefCell<Scene>>,
    pub(super) input: Rc<RefCell<InputState>>,
    pub(super) nav: Rc<RefCell<NavigationGraph>>,
    pub(super) console: Rc<RefCell<ConsoleLogs>>,
    pub(super) camera: Rc<RefCell<Camera>>,
    pub(super) time: Rc<RefCell<Time>>,
}

impl ScriptManager {
    pub fn new(
        scene: Rc<RefCell<Scene>>,
        input: Rc<RefCell<InputState>>,
        nav: Rc<RefCell<NavigationGraph>>,
        console: Rc<RefCell<ConsoleLogs>>,
        camera: Rc<RefCell<Camera>>,
        time: Rc<RefCell<Time>>,
    ) -> Self {
        Self {
            lua: None,
            entity_scripts: HashMap::new(),
            scene,
            input,
            nav,
            console,
            camera,
            time,
        }
    }

    /// Initializes a fresh Lua environment and registers all required namespaces.
    /// `physics` is the live rapier world shared with `GameWorld`; the
    /// `Physics.Raycast`/`Shoot` bindings cast against it so scripts and the
    /// engine hitscan agree. It is `None` until Play builds the world.
    pub fn init_runtime(
        &mut self,
        physics: &Rc<RefCell<Option<crate::physics::PhysicsWorld>>>,
    ) -> Result<(), String> {
        let lua = Lua::new();

        // 1. Override print to write to our console panel
        let console_clone = Rc::clone(&self.console);
        let print_fn = lua
            .create_function(move |_, msg: String| {
                console_clone.borrow_mut().info(msg);
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        lua.globals()
            .set("print", print_fn)
            .map_err(|e| e.to_string())?;

        // 2. Register the whole stable API surface. Every namespace
        //    (`Transform`, `Material`, `Animator`, `Input`, `Scene`, `Navigation`,
        //    `NavMeshAgent`, `Physics`, `Health`, `Time`, `Camera`, and the
        //    dev-only `Debug`) is owned by the `api/` tree — the single surface
        //    shared by gameplay scripts, the console REPL and bot-players. The
        //    live `physics` handle rides along so `Physics.Raycast`/`Shoot` reach
        //    the same rapier world the engine hitscan uses (#31).
        let ctx = ApiCtx {
            scene: Rc::clone(&self.scene),
            input: Rc::clone(&self.input),
            nav: Rc::clone(&self.nav),
            camera: Rc::clone(&self.camera),
            time: Rc::clone(&self.time),
            physics: Rc::clone(physics),
            console: Rc::clone(&self.console),
        };
        api::register(&lua, &ctx)?;

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

    /// Run a Lua chunk against the live runtime. Test-only entry point for the
    /// issue-#5 API coverage; the windowed/headless paths drive scripts through
    /// the lifecycle callbacks instead.
    #[cfg(test)]
    pub(super) fn exec(&self, code: &str) -> Result<(), String> {
        let lua = self.lua.as_ref().ok_or("runtime not initialized")?;
        lua.load(code).exec().map_err(|e| e.to_string())
    }

    /// Whether the live runtime exists yet (only during play / a loaded scenario).
    pub fn is_live(&self) -> bool {
        self.lua.is_some()
    }
}
