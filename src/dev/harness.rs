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
//! A run produces: <out_dir>/results.json + <out_dir>/console.log.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use serde_json::{json, Value};

use crate::app::GameWorld;
use crate::core::input::InputState;
use crate::navigation::NavigationGraph;
use crate::scene::Scene;
use crate::scripting::{ConsoleLogs, LogLevel};

/// The simulation timestep. 60 Hz, fixed — the source of determinism.
pub const FIXED_DT: f32 = 1.0 / 60.0;

/// A single assertion recorded by `Harness.Expect`.
#[derive(Clone)]
pub struct Expectation {
    pub passed: bool,
    pub message: String,
    pub frame: u64,
}

/// The headless harness: owns the world and the run record.
pub struct Harness {
    pub world: Rc<RefCell<GameWorld>>,
    pub console: Rc<RefCell<ConsoleLogs>>,
    pub logs: Vec<String>,
    pub expectations: Vec<Expectation>,
    out_dir: PathBuf,
}

impl Harness {
    /// Build a harness around a freshly populated demo world and enter play mode.
    /// `bot_script` is the enemy brain path (empty to skip scripts).
    pub fn new(out_dir: impl AsRef<Path>, bot_script: &str) -> Self {
        // Headless runs don't boot through main.rs, so seed the bundled default
        // scripts (the Player's controller + enemy brain) into the project workspace
        // here too, exactly as the windowed boot does.
        crate::scene::seed_default_scripts();

        let mut scene = Scene::new();
        super::demo_scene::build(&mut scene, bot_script);

        let nav = NavigationGraph::new(-20.0, 20.0, -20.0, 20.0, 1.0);
        let scene = Rc::new(RefCell::new(scene));
        let input = Rc::new(RefCell::new(InputState::new()));
        let nav = Rc::new(RefCell::new(nav));
        let console = Rc::new(RefCell::new(ConsoleLogs::new()));
        nav.borrow_mut().bake(&scene.borrow());

        let mut world = GameWorld::new(
            Rc::clone(&scene),
            Rc::clone(&input),
            Rc::clone(&nav),
            Rc::clone(&console),
        );
        // Headless harness always runs the simulation (play mode).
        world.is_playing = true;

        Self {
            world: Rc::new(RefCell::new(world)),
            console,
            logs: Vec::new(),
            expectations: Vec::new(),
            out_dir: out_dir.as_ref().to_path_buf(),
        }
    }

    /// Advance the simulation by exactly `n` fixed ticks.
    pub fn step(&self, n: u32) {
        let mut world = self.world.borrow_mut();
        for _ in 0..n {
            world.tick(FIXED_DT);
        }
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

    /// Render the current scene/camera offscreen and write a PNG to `path`.
    ///
    /// Returns `true` if a frame was captured, `false` if no GPU/software adapter
    /// is available (skipped gracefully — never panics). Logs the outcome.
    pub fn screenshot(&mut self, path: impl AsRef<Path>) -> bool {
        let path = path.as_ref().to_path_buf();
        let result = super::screenshot::capture_world(
            &self.world.borrow(),
            &path,
            super::screenshot::DEFAULT_WIDTH,
            super::screenshot::DEFAULT_HEIGHT,
        );
        match result {
            Ok(true) => {
                self.log(format!("Screenshot written: {}", path.display()));
                true
            }
            Ok(false) => {
                self.log(format!(
                    "Screenshot skipped (no GPU adapter): {}",
                    path.display()
                ));
                false
            }
            Err(e) => {
                self.console.borrow_mut().error(format!("[Harness] {e}"));
                false
            }
        }
    }

    /// True if every recorded expectation passed.
    pub fn all_passed(&self) -> bool {
        self.expectations.iter().all(|e| e.passed)
    }

    /// Flush `results.json` + `console.log` into `out_dir`. Returns the results path.
    pub fn write_results(&self) -> std::io::Result<PathBuf> {
        std::fs::create_dir_all(&self.out_dir)?;

        let expects: Vec<Value> = self
            .expectations
            .iter()
            .map(|e| json!({ "passed": e.passed, "message": e.message, "frame": e.frame }))
            .collect();
        let results = json!({
            "frames": self.frame(),
            "passed": self.all_passed(),
            "expectations": expects,
            "logs": self.logs,
            "final_snapshot": self.snapshot(),
        });

        let results_path = self.out_dir.join("results.json");
        std::fs::write(&results_path, serde_json::to_string_pretty(&results)?)?;

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
        std::fs::write(console_path, buf)?;

        Ok(results_path)
    }
}
