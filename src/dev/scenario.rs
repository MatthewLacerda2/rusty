//! src/dev/scenario.rs — Scenario loader / runner
//!
//! A scenario is a normal `.lua` file (tagged dev-only) that drives the headless
//! harness through the SAME observe/act surface an agent uses. The loop is:
//!
//!   1. write   `scenarios/<name>.lua` in the game project
//!   2. run     `cargo run --bin play --features dev -- --project <dir> [--scene <path>] <scenario> <out_dir>`
//!   3. read    `<out_dir>/results.json` + `console.log`
//!
//! It plays the project's startup scene, or the `--scene` named. Example scenario
//! (`Player` stands for an entity of that scene):
//!   local player = Scene.FindEntityByName("Player")
//!   local x0, _, z0 = Transform.GetPosition(player)
//!   Input.Press("W"); Harness.Step(120)            -- walk 2s @ 1/60
//!   local x1, _, z1 = Transform.GetPosition(player)
//!   Harness.Expect(x1 ~= x0 or z1 ~= z0, "W should move the player")
//!
//! Runs in its own Lua VM, separate from the gameplay scripts inside `GameWorld`,
//! with the same API they get (`api::register`) plus the scenario-only `Harness`
//! table (`dev::bridge`), and the same deterministic stdlib (#1408): no `os` /
//! `io`, and `math.random` draws from the world's one seeded `Random` stream, so
//! `results.json` is a pure function of the scenario.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::harness::Harness;
use crate::api;
use crate::core::application::BuildSettings;
use crate::core::settings::decals::DecalLimits;
use crate::core::settings::graphics_options::GraphicsOptions;

/// Default enemy brain used by the default scene when running scenarios.
const DEFAULT_BOT_SCRIPT: &str = crate::scene::default_scene::BOT_SCRIPT;

/// Outcome of a scenario run.
pub struct RunReport {
    pub results_path: PathBuf,
    pub passed: bool,
}

/// Load and run `scenario_path` headlessly, writing results into `out_dir`, on the
/// open project (`core::project`), playing `scene` (project-relative) under the
/// project's `build` settings, default `graphics` options and `decals` budgets: a scenario play-tests
/// the game being built.
pub fn run(
    scenario_path: &Path,
    out_dir: &Path,
    scene: &str,
    build: BuildSettings,
    graphics: GraphicsOptions,
    decals: DecalLimits,
) -> Result<RunReport, String> {
    let harness = Harness::in_user_workspace(out_dir, scene, build, graphics, decals)?;
    run_in(harness, scenario_path)
}

/// [`run`] on a workspace of the run's own, seeded from the engine's bundled
/// scripts (see `Harness::new`) — for tests, which must not read a developer's project (#782).
/// Test seam for `tests/input_harness.rs`.
pub fn run_isolated(scenario_path: &Path, out_dir: &Path) -> Result<RunReport, String> {
    run_in(Harness::new(out_dir, DEFAULT_BOT_SCRIPT), scenario_path)
}

fn run_in(harness: Harness, scenario_path: &Path) -> Result<RunReport, String> {
    let code = std::fs::read_to_string(scenario_path)
        .map_err(|e| format!("Failed to read scenario {}: {}", scenario_path.display(), e))?;
    let harness = Rc::new(RefCell::new(harness));

    // The real API over the harness world's own cells (#1384), held for the whole
    // run: every call borrows a cell only while it runs, and `Harness.Step` ticks
    // the world (whose gameplay VM registers the same surface in its own scope)
    // between calls, never inside one.
    let cells = harness.borrow().world.borrow().script_manager().api_cells();
    let ctx = cells.ctx();
    // The world's one `Random` stream, not a scenario-only one: Unity has one
    // `Random` too, and a scenario's draws are its inputs, as deterministic as
    // the rest of them.
    let lua = crate::scripting::sandboxed_vm()?;
    let exec = lua
        .scope(|scope| {
            api::register(&lua, scope, &ctx).map_err(mlua::Error::RuntimeError)?;
            super::bridge::register(&lua, &harness)?;
            Ok(lua
                .load(&code)
                .set_name(scenario_path.to_string_lossy())
                .exec())
        })
        .map_err(|e| format!("Lua bridge error: {}", e))?;
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
