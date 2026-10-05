//! src/bin/play.rs — headless scenario runner (dev-only, `--features dev`).
//!
//! ```text
//! Usage:
//!   cargo run --bin play --features dev -- [--project <dir>] <scenario.lua> <out_dir>
//! ```
//!
//! The scenario plays the game project at `<dir>` (default `./project`, #829); the
//! two paths are read from where the command was run, before the project opens.
//!
//! Drives `app::GameWorld::tick` at a fixed 1/60 timestep with NO window/GPU, runs
//! the given Lua scenario, and writes <out_dir>/results.json + <out_dir>/console.log.
//! Deterministic: fixed dt + frame-count timers, so reruns are identical.

use std::path::{absolute, PathBuf};
use std::process::exit;

use rusty::core::project::Access;
use rusty::dev::scenario;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let project = rusty::core::project::take_flag(&mut args);
    let (Ok(project), [scenario_path, out_dir]) = (project, args.as_slice()) else {
        eprintln!("usage: play [--project <dir>] <scenario.lua> <out_dir>");
        exit(2);
    };
    // Both are named from the launch directory; opening the project moves it.
    let from_here = |p: &str| absolute(p).unwrap_or_else(|_| PathBuf::from(p));
    let (scenario_path, out_dir) = (from_here(scenario_path), from_here(out_dir));
    let opened = rusty::core::project::open(&rusty::core::project::locate(project), Access::Run);
    if let Err(e) = opened {
        eprintln!("play: cannot open the project: {e}");
        exit(2);
    }

    match scenario::run(&scenario_path, &out_dir) {
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
