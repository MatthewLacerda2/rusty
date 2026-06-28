//! src/dev/scenario.rs — Scenario loader / runner
//!
//! A scenario is a normal `.lua` file (tagged dev-only) that drives the headless
//! harness through the SAME observe/act surface an agent uses. The loop is:
//!
//!   1. write   `project/scenarios/<name>.lua`
//!   2. run     `cargo run --bin play --features dev -- <scenario> <out_dir>`
//!   3. read    `<out_dir>/results.json` + `console.log`
//!
//! Example scenario:
//!   local enemy = Scene.FindEntityByName("Enemy_1")
//!   Input.Press("W"); Harness.Step(120)            -- walk 2s @ 1/60
//!   local _, _, ez = Transform.GetPosition(enemy)
//!   Harness.Expect(ez ~= nil, "the bot should have advanced toward the enemy")
//!
//! Runs in its own Lua VM, separate from the gameplay scripts inside `GameWorld`.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use mlua::Lua;

use super::harness::Harness;

/// Default enemy brain used by the demo scene when running scenarios.
pub const DEFAULT_BOT_SCRIPT: &str = "project/assets/scripts/bot.lua";

/// Outcome of a scenario run.
pub struct RunReport {
    pub results_path: PathBuf,
    pub passed: bool,
}

/// Load and run `scenario_path` headlessly, writing results into `out_dir`.
pub fn run(scenario_path: &Path, out_dir: &Path) -> Result<RunReport, String> {
    let code = std::fs::read_to_string(scenario_path)
        .map_err(|e| format!("Failed to read scenario {}: {}", scenario_path.display(), e))?;

    // Seed the bundled default scripts before deciding what to attach, so the very
    // first run and every rerun see the same files — otherwise the enemy brain would
    // be absent on the first run and present afterwards, breaking byte-determinism.
    crate::scene::seed_default_scripts();

    let bot = if Path::new(DEFAULT_BOT_SCRIPT).exists() {
        DEFAULT_BOT_SCRIPT
    } else {
        ""
    };
    let harness = Rc::new(RefCell::new(Harness::new(out_dir, bot)));

    let lua = Lua::new();
    super::bridge::register(&lua, &harness).map_err(|e| format!("Lua bridge error: {}", e))?;

    let exec = lua
        .load(&code)
        .set_name(scenario_path.to_string_lossy())
        .exec();
    if let Err(e) = &exec {
        harness
            .borrow()
            .console
            .borrow_mut()
            .error(format!("[Scenario error] {}", e));
    }

    let h = harness.borrow();
    let results_path = h
        .write_results()
        .map_err(|e| format!("Failed to write results: {}", e))?;
    Ok(RunReport {
        results_path,
        passed: h.all_passed() && exec.is_ok(),
    })
}
