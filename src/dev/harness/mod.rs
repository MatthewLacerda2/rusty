//! src/dev/harness.rs — Headless deterministic runner (the agent's hands)
//!
//! Builds a `GameWorld` with NO window/GPU, runs the fixed-timestep clock as fast as
//! the CPU allows, and exposes the control surface an agent needs:
//!
//!   Step(n)              advance exactly n fixed ticks (dt = 1/60).
//!   StepUntil(pred, max) advance until a Lua predicate is true or max ticks pass —
//!                        the "skip ticks" primitive: run a whole match, observe ONCE.
//!   Snapshot()           dump world state (entities, camera, play-state) as JSON.
//!   Log / Expect         record observations + pass/fail assertions.
//!
//! Determinism contract: fixed timestep + frame-count timers (no wall-clock reads in
//! the tick), so a scenario replays identically every run.
//!
//! A run produces: <out_dir>/results.json + <out_dir>/console.log + <out_dir>/stats.json.
//! The rendering verbs (`Screenshot`, `Render`) live in `render`.
//!
//! Frame stats (#433): the harness installs the dev-layer timing probe, so every run
//! records CPU ms per stage/system and world counters (render counters too, on the
//! frames a `Screenshot` draws). `results.json` carries only the deterministic counts
//! — it stays byte-identical across replays — and `stats.json` carries everything,
//! wall-clock timings included. `AssertBudget` turns a limit into an expectation.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use serde_json::{json, Value};

use crate::app::GameWorld;
use crate::core::frame_stats::FrameStats;
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::render::TextureSizeReader;
use crate::scene::Scene;
use crate::scripting::{ConsoleLogs, LogLevel};

mod render;

/// The simulation timestep. 60 Hz, fixed — the source of determinism.
pub const FIXED_DT: f32 = 1.0 / 60.0;

/// A single assertion recorded by `Harness.Expect`.
#[derive(Clone)]
pub struct Expectation {
    pub passed: bool,
    pub message: String,
    pub frame: u64,
}

/// Advance `world` one fixed tick unless the game has called `Application.Quit()`.
///
/// Quit **ends a headless run** (#431): once requested, every further step is a no-op,
/// so a scenario's remaining observations read the state the game quit in and
/// `results.json` records `"quit": true`. Returns whether a tick ran.
///
/// Before the tick `ui_textures` reads the UI's texture sizes from the files into
/// the sim, as the window does before each of its ticks (#1221), so a headless
/// layout matches the drawn one. No Lua binding writes the table: Rust code driving
/// a harness may set sizes on `resources.screen.textures_mut()` between steps (only
/// for paths the scene's Images use; see `render::TextureSizeReader`).
pub fn tick_unless_quit(world: &mut GameWorld, ui_textures: &mut TextureSizeReader) -> bool {
    if world.quit_requested() {
        return false;
    }
    {
        let scene = world.scene().borrow();
        ui_textures.refresh_screen(&scene.world, &mut world.resources.screen.borrow_mut());
    }
    world.tick(FIXED_DT);
    // Engine warnings join the console, so a scenario's `console.log` carries them.
    crate::logging::forward(world.console());
    true
}

/// The engine checkout's test-fixture project (#829): the committed scenarios, the
/// bot-player script and build settings, opened with `--project` like any other.
pub const FIXTURE_PROJECT: &str = "tests/fixtures/project";

/// [`FIXTURE_PROJECT`] in the checkout this binary belongs to, from any directory.
pub fn fixture_project() -> PathBuf {
    let engine = crate::core::project::engine_dir();
    engine.parent().unwrap_or(engine).join(FIXTURE_PROJECT)
}

/// The headless harness: owns the world and the run record.
pub struct Harness {
    pub world: Rc<RefCell<GameWorld>>,
    pub console: Rc<RefCell<ConsoleLogs>>,
    pub logs: Vec<String>,
    pub expectations: Vec<Expectation>,
    out_dir: PathBuf,
    /// The renderer every `Harness.Screenshot` in this run draws through (#355 step 5).
    /// A scenario that observes six moments used to build six wgpu devices; now it
    /// builds one, on its first shot, and none at all if it never takes one. The budget
    /// slot it holds is released when the harness is dropped.
    capture: super::capture::CaptureHost,
    /// The run's frame stats, filled by the timing probe and the screenshots.
    pub stats: Rc<RefCell<FrameStats>>,
    /// Reads the UI's texture sizes into the sim before every tick (#1221).
    pub ui_textures: Rc<RefCell<TextureSizeReader>>,
}

impl Harness {
    /// Build a harness around the default scene — the one the editor seeds (#667) —
    /// and enter play mode. `bot_script` is the enemy brain path (empty to skip it).
    ///
    /// The run gets its own workspace, `<out_dir>/workspace`, seeded fresh from the
    /// test-fixture project's scripts ([`fixture_project`]) and the engine's bundled
    /// ones, and the scene's relative script paths resolve there (#782): a test
    /// judges the engine's scripts, never a developer's project.
    pub fn new(out_dir: impl AsRef<Path>, bot_script: &str) -> Self {
        let workspace = Self::workspace_of(&out_dir);
        let samples = fixture_project().join(crate::scene::DEFAULT_SCRIPTS_DEST_DIR);
        crate::scene::seed_workspace(&workspace, Some(&samples));
        Self::in_workspace(out_dir, bot_script, workspace)
    }

    /// [`Harness::new`] on the open project (`core::project`, the working directory)
    /// — what an agent play-testing its game wants (the `play` binary). The bundled
    /// scripts are seeded through the seed manifest, as the windowed boot does.
    pub fn in_user_workspace(out_dir: impl AsRef<Path>, bot_script: &str) -> Self {
        crate::scene::seed_default_scripts();
        Self::in_workspace(out_dir, bot_script, PathBuf::new())
    }

    /// The workspace a harness built in `out_dir` runs in: see [`Harness::new`].
    pub fn workspace_of(out_dir: impl AsRef<Path>) -> PathBuf {
        out_dir.as_ref().join("workspace")
    }

    fn in_workspace(out_dir: impl AsRef<Path>, bot_script: &str, workspace: PathBuf) -> Self {
        // The default scene's texture and shader, seeded into the working directory
        // only when missing; only screenshots read them.
        crate::scene::default_scene::seed_default_assets();

        let mut scene = Scene::new();
        crate::scene::default_scene::build(&mut scene, bot_script);

        // Baked over the scene's own bounds (#452) — the same path the windowed game takes.
        let nav = NavigationGraph::from_scene(&scene);
        let scene = Rc::new(RefCell::new(scene));
        let input = Rc::new(RefCell::new(InputState::new()));
        let nav = Rc::new(RefCell::new(nav));
        let console = Rc::new(RefCell::new(ConsoleLogs::new()));

        super::lua_surface::install_api();
        let mut world = GameWorld::new(
            Rc::clone(&scene),
            Rc::clone(&input),
            Rc::clone(&nav),
            Rc::clone(&console),
        );
        // Headless harness always runs the simulation (play mode).
        world.resources.script_manager.set_workspace_root(workspace);
        world.set_playing(true);
        let stats = super::stats::install(&mut world);

        Self {
            world: Rc::new(RefCell::new(world)),
            console,
            logs: Vec::new(),
            expectations: Vec::new(),
            out_dir: out_dir.as_ref().to_path_buf(),
            capture: super::capture::CaptureHost::new(),
            stats,
            ui_textures: Rc::default(),
        }
    }

    /// Advance the simulation by exactly `n` fixed ticks, or fewer if the game quits.
    pub fn step(&self, n: u32) {
        let (mut world, mut sizes) = (self.world.borrow_mut(), self.ui_textures.borrow_mut());
        for _ in 0..n {
            if !tick_unless_quit(&mut world, &mut sizes) {
                break;
            }
        }
    }

    /// Where the run writes its results (and `Harness.Bench` keeps its report).
    pub fn out_dir(&self) -> &Path {
        &self.out_dir
    }

    /// Current play-mode frame count.
    pub fn frame(&self) -> u64 {
        self.world.borrow().play_frame()
    }

    /// Record a free-form observation line.
    pub fn log(&mut self, msg: String) {
        self.console.borrow_mut().info(format!("[Harness] {}", msg));
        self.logs.push(msg);
    }

    /// Record a pass/fail assertion.
    pub fn expect(&mut self, passed: bool, message: String) {
        let frame = self.frame();
        let tag = if passed { "PASS" } else { "FAIL" };
        self.console
            .borrow_mut()
            .info(format!("[Expect:{}] {}", tag, message));
        self.expectations.push(Expectation {
            passed,
            message,
            frame,
        });
    }

    /// JSON snapshot of the current world state.
    pub fn snapshot(&self) -> Value {
        super::snapshot::snapshot(&self.world.borrow())
    }

    /// Check each `(metric, limit)` budget against the worst frame recorded so far,
    /// recording one expectation per budget. Returns whether all were met.
    pub fn assert_budget(&mut self, budgets: &[(String, f64)]) -> bool {
        let mut all = true;
        for (key, limit) in budgets {
            let checked = self.stats.borrow().check_budget(key, *limit);
            all &= checked.is_ok();
            let (passed, message) = match checked {
                Ok(msg) => (true, msg),
                Err(msg) => (false, msg),
            };
            self.expect(passed, message);
        }
        all
    }

    /// True if every recorded expectation passed.
    pub fn all_passed(&self) -> bool {
        self.expectations.iter().all(|e| e.passed)
    }

    /// Flush `results.json` + `console.log` into `out_dir`. Returns the results path.
    pub fn write_results(&self) -> std::io::Result<PathBuf> {
        // Warnings raised outside a tick (loading the scene, say) join the log first.
        crate::logging::forward(&self.console);
        std::fs::create_dir_all(&self.out_dir)?;

        let expects: Vec<Value> = self
            .expectations
            .iter()
            .map(|e| json!({ "passed": e.passed, "message": e.message, "frame": e.frame }))
            .collect();
        let results = json!({
            "frames": self.frame(),
            "quit": self.world.borrow().quit_requested(),
            "passed": self.all_passed(),
            "expectations": expects,
            "logs": self.logs,
            "final_snapshot": self.snapshot(),
            "stats": self.stats.borrow().to_json(false),
        });

        let results_path = self.out_dir.join("results.json");
        // lint: allow(asset-write) a harness report, not read by a path-keyed cache
        std::fs::write(&results_path, serde_json::to_string_pretty(&results)?)?;

        // Timings differ run to run, so they live beside results.json, not in it.
        let stats = self.stats.borrow().to_json(true);
        // lint: allow(asset-write) a harness report, not read by a path-keyed cache
        std::fs::write(
            self.out_dir.join("stats.json"),
            serde_json::to_string_pretty(&stats)?,
        )?;

        let console_path = self.out_dir.join("console.log");
        let mut buf = String::new();
        for (msg, level) in &self.console.borrow().messages {
            let tag = match level {
                LogLevel::Info => "INFO",
                LogLevel::Warning => "WARN",
                LogLevel::Error => "ERROR",
            };
            buf.push_str(&format!("[{}] {}\n", tag, msg));
        }
        // lint: allow(asset-write) a harness report, not read by a path-keyed cache
        std::fs::write(console_path, buf)?;

        Ok(results_path)
    }
}
