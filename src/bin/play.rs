//! src/bin/play.rs — headless scenario runner (dev-only, `--features dev`).
//!
//! ```text
//! Usage:
//!   cargo run --bin play --features dev -- [--project <dir>] [--scene <path>] [--size <W>x<H>] [--record-audio <path>] <scenario.lua> <out_dir>
//!   cargo run --bin play --features dev -- [--project <dir>] [--record-audio <path>] --demo <file> [scenario.lua] <out_dir>
//! ```
//!
//! The scenario plays the game project at `<dir>` (default `./project`, #829); the
//! two paths are read from where the command was run, before the project opens.
//! The scene played is the project's startup scene (`project.rusty`'s
//! `build.startup_scene`), as the standalone player boots, or `--scene <path>`,
//! relative to the project like the startup scene (#1343). `--size <W>x<H>` is the
//! run's render size, as a Game view resolution: every screenshot, post effect and
//! UI layout is sized to it (default 1280x720, #1583).
//! `--record-audio <path>` captures the whole run's final audio mix to a WAV
//! (#1579), named from the launch directory like the other paths.
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

use rusty::core::project::{boot_scene, Access};
use rusty::core::settings::video::parse_size;
use rusty::dev::{demo, scenario};

const USAGE: &str =
    "usage: play [--project <dir>] [--scene <path>] [--size <W>x<H>] [--record-audio <path>] <scenario.lua> <out_dir>
       play [--project <dir>] [--record-audio <path>] --demo <file> [scenario.lua] <out_dir>";

fn main() {
    rusty::logging::init();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let project = rusty::core::project::take_flag(&mut args);
    let scene = rusty::core::project::take_value(&mut args, "--scene");
    let demo = rusty::core::project::take_value(&mut args, "--demo");
    let size = rusty::core::project::take_value(&mut args, "--size")
        .and_then(|size| size.as_deref().map(parse_size).transpose());
    let audio = rusty::core::project::take_value(&mut args, "--record-audio");
    let (project, scene, size, audio, demo) = match (project, scene, size, audio, demo) {
        (Ok(project), Ok(scene), Ok(size), Ok(audio), Ok(demo)) => {
            (project, scene, size, audio, demo)
        }
        (Err(e), ..) | (_, Err(e), ..) | (_, _, Err(e), ..) | (.., Err(e), _) | (.., Err(e)) => {
            eprintln!("play: {e}\n{USAGE}");
            exit(2);
        }
    };
    let (scenario_path, out_dir) = positional(&args, demo.is_some());
    // The demo recorded both: the scene Play started in, and the Game view size
    // the sim laid the UI out on.
    let fixed = [
        ("--scene", scene.is_some(), "scene"),
        ("--size", size.is_some(), "size"),
    ];
    if let Some((flag, _, what)) = fixed.iter().find(|f| demo.is_some() && f.1) {
        eprintln!(
            "play: {flag} cannot change a --demo replay: it plays the {what} it recorded\n{USAGE}"
        );
        exit(2);
    }
    let demo = demo.map(|d| from_here(&d));
    let audio = audio.map(|path| from_here(&path));
    let opened = open_project(project);
    let startup = &opened.settings.build.startup_scene;
    let scene = scene.unwrap_or_else(|| boot_scene(&opened.root, startup, Access::Run));

    let run = scenario::Run {
        size,
        record_audio: audio.as_deref(),
    };
    let ran = match (&demo, &scenario_path) {
        (Some(demo), scenario_path) => demo::replay(demo, scenario_path.as_deref(), &out_dir, run),
        (None, Some(path)) => scenario::run(path, &out_dir, &scene, opened.settings, run),
        (None, None) => Err(USAGE.to_string()),
    };
    report(ran);
}

/// Open the project to run it; exits with status 2 when it can't be opened.
fn open_project(project: Option<PathBuf>) -> rusty::core::project::Opened {
    let opened = rusty::core::project::open(&rusty::core::project::locate(project), Access::Run);
    opened.unwrap_or_else(|e| {
        eprintln!("play: cannot open the project: {e}");
        exit(2);
    })
}

/// Paths are named from the launch directory; opening the project moves it.
fn from_here(p: &str) -> PathBuf {
    absolute(p).unwrap_or_else(|_| PathBuf::from(p))
}

/// The scenario (optional with `--demo`) and the output folder, or the usage.
fn positional(args: &[String], demo: bool) -> (Option<PathBuf>, PathBuf) {
    match args {
        [scenario, out] => (Some(from_here(scenario)), from_here(out)),
        [out] if demo => (None, from_here(out)),
        _ => {
            eprintln!("{USAGE}");
            exit(2);
        }
    }
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
