use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::harness::Harness;
use rusty_core::project::ProjectSettings;

const DEFAULT_BOT_SCRIPT: &str = rusty_edit::default_scene::BOT_SCRIPT;

pub struct RunReport {
    pub results_path: PathBuf,
    pub passed: bool,
    pub demos: Vec<PathBuf>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Run<'a> {
    pub size: Option<(u32, u32)>,
    pub record_audio: Option<&'a Path>,
}

pub fn run(
    scenario_path: &Path,
    out_dir: &Path,
    scene: &str,
    settings: ProjectSettings,
    run: Run<'_>,
) -> Result<RunReport, String> {
    let mut harness = Harness::in_user_workspace(out_dir, scene, settings)?;
    if let Some((width, height)) = run.size {
        harness.set_size(width, height);
    }
    if let Some(path) = run.record_audio {
        harness.record_audio(Some(path))?;
    }
    super::demo::record_project(&mut harness.world.borrow_mut());
    run_file(harness, scenario_path)
}

pub fn run_isolated(scenario_path: &Path, out_dir: &Path) -> Result<RunReport, String> {
    run_file(Harness::new(out_dir, DEFAULT_BOT_SCRIPT), scenario_path)
}

pub fn run_file(harness: Harness, scenario_path: &Path) -> Result<RunReport, String> {
    let code = std::fs::read_to_string(scenario_path)
        .map_err(|e| format!("Failed to read scenario {}: {}", scenario_path.display(), e))?;
    run_code(harness, &code, &scenario_path.to_string_lossy())
}

pub fn run_code(harness: Harness, code: &str, name: &str) -> Result<RunReport, String> {
    let harness = Rc::new(RefCell::new(harness));

    let cells = harness.borrow().world.borrow().script_manager().api_cells();
    let lua = rusty_script::scripting::sandboxed_vm()?;
    cells.install(&lua)?;
    harness
        .borrow()
        .world
        .borrow()
        .script_manager()
        .install_file_access(&lua)?;
    let bridge_error = |e: mlua::Error| format!("Lua bridge error: {}", e);
    super::bridge::register(&lua, &harness).map_err(bridge_error)?;
    let journal = harness.borrow().world.borrow().script_manager().journal();
    if journal.borrow().is_recording() {
        super::demo::journal_calls(&lua, &journal).map_err(bridge_error)?;
    }
    let exec = lua.load(code).set_name(name).exec();
    if let Err(e) = &exec {
        harness
            .borrow()
            .console
            .borrow_mut()
            .error(format!("[Scenario error] {}", e));
    }

    super::demo::conclude(&mut harness.borrow_mut());
    let h = harness.borrow();
    let demos = (h.world.borrow_mut().tap_mut::<super::demo::Recorder>())
        .map(|recorder| recorder.written().to_vec())
        .unwrap_or_default();
    let results_path = h
        .write_results()
        .map_err(|e| format!("Failed to write results: {}", e))?;
    Ok(RunReport {
        results_path,
        passed: h.all_passed() && exec.is_ok(),
        demos,
    })
}
