//! src/bin/play.rs — headless scenario runner (dev-only, `--features dev`).
//!
//! ```text
//! Usage:
//!   cargo run --bin play --features dev -- [--project <dir>] [--scene <path>] <scenario.lua> <out_dir>
//! ```
//!
//! The scenario plays the game project at `<dir>` (default `./project`, #829); the
//! two paths are read from where the command was run, before the project opens.
//! The scene played is the project's startup scene (`project.rusty`'s
//! `build.startup_scene`), as the standalone player boots, or `--scene <path>`,
//! relative to the project like the startup scene (#1343).
//!
//! Drives `app::GameWorld::tick` at a fixed 1/60 timestep with NO window/GPU, runs
//! the given Lua scenario, and writes <out_dir>/results.json + <out_dir>/console.log.
//! Deterministic: fixed dt + frame-count timers, so reruns are identical.

use std::path::{absolute, PathBuf};
use std::process::exit;

use rusty::core::project::{Access, ProjectFile};
use rusty::dev::scenario;

const USAGE: &str = "usage: play [--project <dir>] [--scene <path>] <scenario.lua> <out_dir>";

fn main() {
    rusty::logging::init();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let project = rusty::core::project::take_flag(&mut args);
    let scene = take_scene(&mut args);
    let (Ok(project), Ok(scene), [scenario_path, out_dir]) = (project, scene, args.as_slice())
    else {
        eprintln!("{USAGE}");
        exit(2);
    };
    // Both are named from the launch directory; opening the project moves it.
    let from_here = |p: &str| absolute(p).unwrap_or_else(|_| PathBuf::from(p));
    let (scenario_path, out_dir) = (from_here(scenario_path), from_here(out_dir));
    let opened = rusty::core::project::open(&rusty::core::project::locate(project), Access::Run);
    let opened = match opened {
        Ok(opened) => opened,
        Err(e) => {
            eprintln!("play: cannot open the project: {e}");
            exit(2);
        }
    };
    let build = ProjectFile::build_or_default(&opened.root, "play");
    let scene = scene.unwrap_or_else(|| build.startup_scene.clone());

    match scenario::run(&scenario_path, &out_dir, &scene, build) {
        Ok(report) => {
            println!(
                "play: wrote {} (passed: {})",
                report.results_path.display(),
                report.passed
            );
            if !report.passed {
                exit(1);
            }
        }
        Err(e) => {
            eprintln!("play: {}", e);
            exit(2);
        }
    }
}

/// Take `--scene <path>` (or `--scene=<path>`) out of `args`; `Ok(None)` when absent.
fn take_scene(args: &mut Vec<String>) -> Result<Option<String>, ()> {
    let Some(at) = args
        .iter()
        .position(|a| a == "--scene" || a.starts_with("--scene="))
    else {
        return Ok(None);
    };
    let flag = args.remove(at);
    if let Some(path) = flag.strip_prefix("--scene=") {
        return Ok(Some(path.to_string()));
    }
    if at >= args.len() || args[at].starts_with("--") {
        return Err(());
    }
    Ok(Some(args.remove(at)))
}
