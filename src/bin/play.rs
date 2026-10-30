//! src/bin/play.rs — headless scenario runner (dev-only, `--features dev`).
//!
//! ```text
//! Usage:
//!   cargo run --bin play --features dev -- [--project <dir>] [--scene <path>] [--size <W>x<H>] <scenario.lua> <out_dir>
//!   cargo run --bin play --features dev -- [--project <dir>] --demo <file> [scenario.lua] <out_dir>
//! ```
//!
//! The scenario plays the game project at `<dir>` (default `./project`, #829); the
//! two paths are read from where the command was run, before the project opens.
//! The scene played is the project's startup scene (`project.rusty`'s
//! `build.startup_scene`), as the standalone player boots, or `--scene <path>`,
//! relative to the project like the startup scene (#1343). `--size <W>x<H>` is the
//! run's render size, as a Game view resolution: every screenshot, post effect and
//! UI layout is sized to it (default 1280x720, #1583).
//!
//! Drives `app::GameWorld::tick` at a fixed 1/60 timestep with NO window/GPU, runs
//! the given Lua scenario, and writes <out_dir>/results.json + <out_dir>/console.log.
//! Deterministic: fixed dt + frame-count timers, so reruns are identical.
//!
//! Every run is recorded as a demo in the project's `saved/demos/` (#1581).
//! `--demo <file>` replays one instead, on the scene and settings it recorded:
//! the whole demo, or as far as the scenario given on top steps it. The replay
//! compares the world against the demo's digests and fails the run at the first
//! that differs (`dev::demo`).

use std::path::{absolute, PathBuf};
use std::process::exit;

use rusty::core::project::Access;
use rusty::core::settings::video::parse_size;
use rusty::dev::{demo, scenario};

const USAGE: &str =
    "usage: play [--project <dir>] [--scene <path>] [--size <W>x<H>] <scenario.lua> <out_dir>
       play [--project <dir>] --demo <file> [scenario.lua] <out_dir>";

fn main() {
    rusty::logging::init();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let project = rusty::core::project::take_flag(&mut args);
    let scene = rusty::core::project::take_value(&mut args, "--scene");
    let demo = rusty::core::project::take_value(&mut args, "--demo");
    let size = rusty::core::project::take_value(&mut args, "--size")
        .and_then(|size| size.as_deref().map(parse_size).transpose());
    let (project, scene, size, demo) = match (project, scene, size, demo) {
        (Ok(project), Ok(scene), Ok(size), Ok(demo)) => (project, scene, size, demo),
        (Err(e), ..) | (_, Err(e), ..) | (_, _, Err(e), _) | (.., Err(e)) => {
            eprintln!("play: {e}\n{USAGE}");
            exit(2);
        }
    };
    // Paths are named from the launch directory; opening the project moves it.
    let from_here = |p: &str| absolute(p).unwrap_or_else(|_| PathBuf::from(p));
    let (scenario_path, out_dir) = match (args.as_slice(), &demo) {
        ([scenario, out], _) => (Some(from_here(scenario)), from_here(out)),
        ([out], Some(_)) => (None, from_here(out)),
        _ => {
            eprintln!("{USAGE}");
            exit(2);
        }
    };
    if demo.is_some() && size.is_some() {
        // The sim lays the UI out on the Game view size, which the demo recorded.
        eprintln!(
            "play: --size cannot change a --demo replay: it plays at the size it recorded\n{USAGE}"
        );
        exit(2);
    }
    let demo = demo.map(|d| from_here(&d));
    let opened = rusty::core::project::open(&rusty::core::project::locate(project), Access::Run);
    let opened = match opened {
        Ok(opened) => opened,
        Err(e) => {
            eprintln!("play: cannot open the project: {e}");
            exit(2);
        }
    };
    let scene = scene.unwrap_or_else(|| opened.settings.build.startup_scene.clone());

    let ran = match (&demo, &scenario_path) {
        (Some(demo), scenario_path) => demo::replay(demo, scenario_path.as_deref(), &out_dir),
        (None, Some(path)) => scenario::run(path, &out_dir, &scene, opened.settings, size),
        (None, None) => Err(USAGE.to_string()),
    };
    report(ran);
}

/// Print where the run wrote its results, and exit non-zero unless it passed.
fn report(ran: Result<scenario::RunReport, String>) {
    match ran {
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
