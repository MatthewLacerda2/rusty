use mlua::{Lua, RegistryKey};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::api::ApiScopedCtx;
use crate::core::input::InputState;
use crate::core::random::Random;
use crate::navigation::NavigationGraph;
use crate::scene::Camera;
use crate::scene::Scene;
use crate::time::Time;
use crate::ui::EventSystem;

use super::console::ConsoleLogs;
use super::shared::ApiCells;

/// One live script instance: the Lua registry key of its lifecycle table plus
/// the per-instance lifecycle flags. `awoken` / `started` record that `Awake` /
/// `Start` already fired, so each fires exactly once per instance (#322).
/// `enabled_last` is the enable/disable edge tracker (#323): the entity's
/// `active` state as of the last transition sweep, diffed each tick so
/// `OnEnable` fires on the rising edge and `OnDisable` on the falling one,
/// exactly once per edge.
pub(super) struct ScriptInstance {
    pub(super) table: RegistryKey,
    pub(super) awoken: bool,
    pub(super) started: bool,
    pub(super) enabled_last: bool,
}

pub struct ScriptManager {
    pub(super) lua: Option<Lua>,
    /// One live instance per attached script, keyed by `(entity_id,
    /// script_index)`. An entity can carry many scripts (#83); each keeps its own
    /// state, and the index is the slot in the entity's `scripts` vec so two
    /// scripts on one entity never collide. A `BTreeMap` so every dispatch loop
    /// iterates in ascending `(entity, script index)` order — the determinism
    /// discipline replays rely on — without re-sorting per hook.
    pub(super) entity_scripts: BTreeMap<(u32, usize), ScriptInstance>,
    /// Every `(entity_id, script_index)` slot a load was ever attempted for,
    /// including failures — so the per-tick spawn scan (`load_new_scripts`)
    /// never retries (and re-logs) a script that failed to compile.
    pub(super) load_attempted: BTreeSet<(u32, usize)>,
    /// Every cell the `api::` surface borrows, shared with `Resources` and the
    /// platform layer; a second VM over the same world clones it (#1384).
    pub(super) cells: ApiCells,
    /// The workspace root a scene's relative script paths resolve against (#782).
    /// Empty — the process's working directory — for the editor and the player;
    /// the headless harness points it at a workspace it seeded itself.
    workspace: PathBuf,
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
            entity_scripts: BTreeMap::new(),
            load_attempted: BTreeSet::new(),
            cells: ApiCells::new(scene, input, nav, console, camera, time),
            workspace: PathBuf::new(),
        }
    }

    /// Resolve the scene's relative script paths, and `require`d modules, against
    /// `root` from now on.
    pub fn set_workspace_root(&mut self, root: impl Into<PathBuf>) {
        self.workspace = root.into();
        if let Some(lua) = &self.lua {
            super::sandbox::install_file_access(lua, &self.workspace, &self.cells.console).ok();
        }
    }

    /// Where the script at `path` lives: `path` under the workspace root, or
    /// `path` itself when it is absolute.
    pub fn resolve_script(&self, path: &str) -> PathBuf {
        self.workspace.join(Path::new(path))
    }

    /// Initializes a fresh Lua environment. API namespaces are NOT registered
    /// here; they are registered per-evaluation inside `lua.scope(...)` in
    /// `lifecycle.rs`, so closures capture `&RefCell<T>` references (no
    /// `Rc::clone` in the `api/` tree).
    pub fn init_runtime(&mut self) -> Result<(), String> {
        // Deterministic sandbox (#443): no `os`/`io`, `math.random` over the
        // seeded RNG — which restarts from its default seed with every fresh VM.
        let lua = super::sandbox::new_sim_vm()?;
        super::sandbox::install_file_access(&lua, &self.workspace, &self.cells.console)?;
        *self.cells.random.borrow_mut() = Random::default();
        *self.cells.event_system.borrow_mut() = EventSystem::default();

        // Override print to write to our console panel. This stays as a
        // `lua.create_function` (static closure) because `print` is in the
        // scripting layer, not the api/ tree — the Rc borrow here is fine.
        let console_clone = Rc::clone(&self.cells.console);
        let print_fn = lua
            .create_function(move |_, msg: String| {
                console_clone.borrow_mut().info(msg);
                Ok(())
            })
            .map_err(|e| e.to_string())?;
        lua.globals()
            .set("print", print_fn)
            .map_err(|e| e.to_string())?;

        self.lua = Some(lua);
        self.entity_scripts.clear();
        self.load_attempted.clear();
        self.cells.timers.borrow_mut().clear();

        Ok(())
    }

    /// Build the scoped context from this manager's resource cells.
    ///
    /// The returned struct borrows through the `Rc` smart pointers, tying
    /// references to the caller's lifetime. Used inside `lua.scope(...)` blocks in
    /// `lifecycle.rs` so the borrow can't outlive the scope.
    pub(super) fn make_ctx(&self) -> ApiScopedCtx<'_> {
        self.cells.ctx()
    }

    /// Stops and clears the script environment
    pub fn shutdown(&mut self) {
        self.entity_scripts.clear();
        self.load_attempted.clear();
        self.cells.timers.borrow_mut().clear();
        self.lua = None;
    }

    /// Run a Lua chunk against the live runtime. Test-only entry point for the
    /// issue-#5 API coverage; the windowed/headless paths drive scripts through
    /// the lifecycle callbacks instead.
    #[cfg(test)]
    pub(super) fn exec(&self, code: &str) -> Result<(), String> {
        use crate::api;
        let lua = self.lua.as_ref().ok_or("runtime not initialized")?;
        let ctx = self.make_ctx();
        lua.scope(|scope| {
            api::register(lua, scope, &ctx).map_err(mlua::Error::RuntimeError)?;
            lua.load(code).exec()
        })
        .map_err(|e| e.to_string())
    }

    /// Whether the live runtime exists: during Play, and outside it on a host that
    /// brought the edit-mode runtime up (the editor, the headless session).
    pub fn is_live(&self) -> bool {
        self.lua.is_some()
    }
}
