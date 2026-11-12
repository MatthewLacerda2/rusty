use mlua::{Lua, RegistryKey};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::components::ScriptComponent;
use crate::core::input::InputState;
use crate::core::random::{Random, DEFAULT_SEED};
use crate::ecs::Cursor;
use crate::navigation::NavigationGraph;
use crate::scene::Camera;
use crate::scene::Scene;
use crate::time::Time;
use crate::ui::EventSystem;

use super::console::ConsoleLogs;
use super::journal::HostJournal;
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
    /// including failures — so the spawn queue's drain (`load_new_scripts`)
    /// never retries (and re-logs) a script that failed to compile.
    pub(super) load_attempted: BTreeSet<(u32, usize)>,
    /// Each script file read and compiled once for this VM (#1710).
    pub(super) classes: super::loader::ScriptClasses,
    /// Where the spawn queue — the World's `scripts` change log — was last read
    /// (#1710); `None` until this VM's first drain, which scans every entity.
    pub(super) spawn_cursor: Option<Cursor<Vec<ScriptComponent>>>,
    /// Entities whose load records were forgotten: the next drain looks at them
    /// whether or not the log stamped them.
    pub(super) requeued: BTreeSet<u32>,
    /// Every cell the `api::` surface borrows, shared with `Resources` and the
    /// platform layer; a second VM over the same world clones it (#1384).
    pub(super) cells: ApiCells,
    /// The workspace root a scene's relative script paths resolve against (#782).
    /// Empty — the process's working directory — for the editor and the player;
    /// the headless harness points it at a workspace it seeded itself.
    workspace: PathBuf,
    /// Where `Random` starts with every fresh VM (#443): [`DEFAULT_SEED`] unless a
    /// demo replay asks for the seed it recorded (#1581).
    play_seed: u64,
    /// A host's acts on the world while a demo records (#1581); [`Self::eval`]
    /// notes every line it runs.
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

    /// Resolve the scene's relative script paths, and `require`d modules, against
    /// `root` from now on.
    pub fn set_workspace_root(&mut self, root: impl Into<PathBuf>) {
        self.workspace = root.into();
        if let Some(lua) = &self.lua {
            self.install_file_access(lua).ok();
        }
    }

    /// Make `lua`'s `require`, `dofile` and `loadfile` read against this
    /// manager's workspace, warning in its console on a file outside it (#1062).
    /// The gameplay VM and the `play` harness's scenario VM both call it (#1717),
    /// so a game module's `require` / `dofile` / `loadfile` resolve in a scenario
    /// as in Play. Only file access is shared: a scenario's `print` is Lua's own,
    /// to stdout, not the gameplay VM's `console.log`.
    pub(crate) fn install_file_access(&self, lua: &Lua) -> Result<(), String> {
        super::sandbox::install_file_access(lua, &self.workspace, &self.cells.console)
    }

    /// Where the script at `path` lives: `path` under the workspace root, or
    /// `path` itself when it is absolute.
    pub fn resolve_script(&self, path: &str) -> PathBuf {
        self.workspace.join(Path::new(path))
    }

    /// Initializes a fresh Lua environment and installs the whole API surface on
    /// it, once (#1709): the shared cells are attached as the VM's app data and
    /// every namespace is registered as plain functions that reach them through
    /// it (`api::ApiCell`), so no `api/` closure captures an `Rc`. The surface
    /// then stays for the VM's life, and so does a script's change to one of its
    /// tables.
    pub fn init_runtime(&mut self) -> Result<(), String> {
        // Deterministic sandbox (#443): no `os`/`io`, `math.random` over the
        // seeded RNG — which restarts from its default seed with every fresh VM.
        let lua = super::sandbox::new_sim_vm()?;
        self.install_file_access(&lua)?;
        *self.cells.random.borrow_mut() = Random::new(self.play_seed);
        *self.cells.event_system.borrow_mut() = EventSystem::default();

        // Override print to write to our console panel. `print` is the scripting
        // layer's, not the api/ tree's, so it may hold its own `Rc`.
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

    /// Re-attach the cells to the live VM after a `set_*` injector swapped one, so
    /// the surface's next call reaches the new cell.
    pub(super) fn reattach_cells(&self) {
        if let Some(lua) = &self.lua {
            self.cells.attach(lua);
        }
    }

    /// Forget what this VM compiled and where its spawn queue stood: the next
    /// drain scans every entity and reads every script afresh.
    fn forget_loads(&mut self) {
        self.classes = Default::default();
        self.spawn_cursor = None;
        self.requeued.clear();
    }

    /// Stops and clears the script environment
    pub fn shutdown(&mut self) {
        self.entity_scripts.clear();
        self.load_attempted.clear();
        self.forget_loads();
        self.cells.timers.borrow_mut().clear();
        *self.cells.queries.borrow_mut() = Default::default();
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

    /// The seed every fresh VM's `Random` starts from.
    pub fn play_seed(&self) -> u64 {
        self.play_seed
    }

    /// Start every fresh VM's `Random` from `seed`: a demo replays under the seed
    /// it was recorded with (#1581). Takes effect at the next Play.
    pub fn set_play_seed(&mut self, seed: u64) {
        self.play_seed = seed;
    }

    /// Where `Random` stands now, for a demo's state digest.
    pub fn random_state(&self) -> u64 {
        self.cells.random.borrow().state()
    }

    /// The host-act journal a demo records from (#1581).
    pub fn journal(&self) -> Rc<RefCell<HostJournal>> {
        Rc::clone(&self.journal)
    }

    /// Whether the live runtime exists: during Play, and outside it on a host that
    /// brought the edit-mode runtime up (the editor, the headless session).
    pub fn is_live(&self) -> bool {
        self.lua.is_some()
    }
}
