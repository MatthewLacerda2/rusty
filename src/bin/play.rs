//! src/bin/play.rs — headless scenario runner (dev-only, `--features dev`).
//!
//! Usage:
//!   cargo run --bin play --features dev -- <scenario.lua> <out_dir>
//!
//! Drives `app::GameWorld::tick` at a fixed 1/60 timestep with NO window/GPU, runs
//! the given Lua scenario, and writes <out_dir>/results.json + <out_dir>/console.log.
//! Deterministic: fixed dt + frame-count timers, so reruns are identical.

use std::path::Path;
use std::process::exit;

use rusty::dev::scenario;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        eprintln!("usage: play <scenario.lua> <out_dir>");
        exit(2);
    }
    let scenario_path = Path::new(&args[0]);
    let out_dir = Path::new(&args[1]);

    match scenario::run(scenario_path, out_dir) {
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
