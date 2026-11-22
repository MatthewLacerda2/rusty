use mlua::{Lua, RegistryKey};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use rusty_components::ScriptComponent;
use rusty_core::input::InputState;
use rusty_core::random::{Random, DEFAULT_SEED};
use rusty_ecs::Cursor;
use rusty_navigation::NavigationGraph;
use rusty_scene::Camera;
use rusty_scene::Scene;
use rusty_time::Time;
use rusty_ui::EventSystem;

use super::console::ConsoleLogs;
use super::journal::HostJournal;
use super::shared::ApiCells;

pub(super) struct ScriptInstance {
    pub(super) table: RegistryKey,
    pub(super) awoken: bool,
    pub(super) started: bool,
    /// `OnEnable` fired and no `OnDisable` since; the sweeps diff it against the entity's
    /// active state.
    pub(super) enabled_last: bool,
}

pub struct ScriptManager {
    pub(super) lua: Option<Lua>,
    /// Keyed by entity and slot in its `scripts`. A `BTreeMap`: every dispatch runs in key
    /// order, which replays rely on.
    pub(super) entity_scripts: BTreeMap<(u32, usize), ScriptInstance>,
    pub(super) load_attempted: BTreeSet<(u32, usize)>,
    pub(super) classes: super::loader::ScriptClasses,
    pub(super) spawn_cursor: Option<Cursor<Vec<ScriptComponent>>>,
    pub(super) requeued: BTreeSet<u32>,
    pub(super) cells: ApiCells,
    /// Empty, the working directory, for the editor and the player; the headless harness
    /// sets its own.
    pub(super) workspace: PathBuf,
    play_seed: u64,
    pub(super) journal: Rc<RefCell<HostJournal>>,
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
            classes: Default::default(),
            spawn_cursor: None,
            requeued: BTreeSet::new(),
            cells: ApiCells::new(scene, input, nav, console, camera, time),
            workspace: PathBuf::new(),
            play_seed: DEFAULT_SEED,
            journal: Rc::default(),
        }
    }

    pub fn set_workspace_root(&mut self, root: impl Into<PathBuf>) {
        self.workspace = root.into();
        if let Some(lua) = &self.lua {
            self.install_file_access(lua).ok();
        }
    }

    pub fn install_file_access(&self, lua: &Lua) -> Result<(), String> {
        super::sandbox::install_file_access(lua, &self.workspace, &self.cells.console)
    }

    pub fn resolve_script(&self, path: &str) -> PathBuf {
        self.workspace.join(Path::new(path))
    }

    pub fn init_runtime(&mut self) -> Result<(), String> {
        let lua = super::sandbox::new_sim_vm()?;
        self.install_file_access(&lua)?;
        *self.cells.random.borrow_mut() = Random::new(self.play_seed);
        *self.cells.event_system.borrow_mut() = EventSystem::default();

        // This `print` is not the `api/` surface's, which reaches cells as app data: no
        // injector swaps the console cell, so holding its `Rc` stays safe.
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

        self.cells.install(&lua)?;
        self.entity_scripts.clear();
        self.load_attempted.clear();
        self.forget_loads();
        self.lua = Some(lua);
        self.cells.timers.borrow_mut().clear();
        *self.cells.queries.borrow_mut() = Default::default();

        Ok(())
    }

    pub(super) fn reattach_cells(&self) {
        if let Some(lua) = &self.lua {
            self.cells.attach(lua);
        }
    }

    fn forget_loads(&mut self) {
        self.classes = Default::default();
        self.spawn_cursor = None;
        self.requeued.clear();
    }

    pub fn shutdown(&mut self) {
        self.entity_scripts.clear();
        self.load_attempted.clear();
        self.forget_loads();
        self.cells.timers.borrow_mut().clear();
        *self.cells.queries.borrow_mut() = Default::default();
        self.lua = None;
    }

    #[cfg(test)]
    pub(super) fn exec(&self, code: &str) -> Result<(), String> {
        let lua = self.lua.as_ref().ok_or("runtime not initialized")?;
        lua.load(code).exec().map_err(|e| e.to_string())
    }

    pub fn play_seed(&self) -> u64 {
        self.play_seed
    }

    /// Takes effect at the next `init_runtime`.
    pub fn set_play_seed(&mut self, seed: u64) {
        self.play_seed = seed;
    }

    pub fn random_state(&self) -> u64 {
        self.cells.random.borrow().state()
    }

    pub fn journal(&self) -> Rc<RefCell<HostJournal>> {
        Rc::clone(&self.journal)
    }

    pub fn is_live(&self) -> bool {
        self.lua.is_some()
    }
}
