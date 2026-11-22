use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use serde_json::{json, Value};

use rusty_app::GameWorld;
use rusty_core::frame_stats::FrameStats;
use rusty_script::scripting::{ConsoleLogs, LogLevel};

pub mod audio;
mod build;
mod render;

/// Seconds per sim tick.
pub const FIXED_DT: f32 = 1.0 / 60.0;

#[derive(Clone)]
pub struct Expectation {
    pub passed: bool,
    pub message: String,
    pub frame: u64,
}

pub fn tick_unless_quit(world: &mut GameWorld) -> bool {
    if world.quit_requested() {
        return false;
    }
    world.tick(FIXED_DT);
    rusty_logging::forward(world.console());
    true
}

pub const FIXTURE_PROJECT: &str = "tests/fixtures/project";

pub fn fixture_project() -> PathBuf {
    let engine = rusty_core::project::engine_dir();
    let root = engine.parent().unwrap_or(engine);
    root.join(FIXTURE_PROJECT)
}

pub struct Harness {
    pub world: Rc<RefCell<GameWorld>>,
    pub console: Rc<RefCell<ConsoleLogs>>,
    pub logs: Vec<String>,
    pub expectations: Vec<Expectation>,
    out_dir: PathBuf,
    capture: super::capture::CaptureHost,
    pub stats: Rc<RefCell<FrameStats>>,
}

impl Harness {
    pub fn step(&self, n: u32) {
        let mut world = self.world.borrow_mut();
        for _ in 0..n {
            if !tick_unless_quit(&mut world) {
                break;
            }
        }
    }

    pub fn out_dir(&self) -> &Path {
        &self.out_dir
    }

    pub fn frame(&self) -> u64 {
        self.world.borrow().play_frame()
    }

    pub fn log(&mut self, msg: String) {
        self.console.borrow_mut().info(format!("[Harness] {}", msg));
        self.logs.push(msg);
    }

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

    pub fn snapshot(&self) -> Value {
        super::snapshot::snapshot(&self.world.borrow())
    }

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

    pub fn all_passed(&self) -> bool {
        self.expectations.iter().all(|e| e.passed)
    }

    pub fn write_results(&self) -> std::io::Result<PathBuf> {
        self.finish_audio();
        rusty_logging::forward(&self.console);
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
            "engine": rusty_core::project::engine_content::stamp(),
        });

        let results_path = self.out_dir.join("results.json");
        // lint: allow(asset-write) a harness report, not read by a path-keyed cache
        // lint: allow(stored-format) a run report for the reader, never loaded back
        std::fs::write(&results_path, serde_json::to_string_pretty(&results)?)?;

        // With timings: they differ run to run, so only `stats.json` has them, never
        // `results.json`, which a replay must write byte for byte.
        let stats = self.stats.borrow().to_json(true);
        // lint: allow(asset-write) a harness report, not read by a path-keyed cache
        // lint: allow(stored-format) a run report for the reader, never loaded back
        std::fs::write(
            self.out_dir.join("stats.json"),
            serde_json::to_string_pretty(&stats)?,
        )?;

        let console_path = self.out_dir.join("console.log");
        // lint: allow(asset-write) a harness report, not read by a path-keyed cache
        // lint: allow(stored-format) the console log, plain text
        std::fs::write(console_path, self.console_text())?;

        Ok(results_path)
    }

    pub fn console_text(&self) -> String {
        let mut buf = String::new();
        for (msg, level) in &self.console.borrow().messages {
            let tag = match level {
                LogLevel::Info => "INFO",
                LogLevel::Warning => "WARN",
                LogLevel::Error => "ERROR",
            };
            buf.push_str(&format!("[{}] {}\n", tag, msg));
        }
        buf
    }
}
