//! src/bin/bench_scenes.rs — bench a game project's own scenes (dev-only, `make bench-scenes`).
//!
//! ```text
//! Usage:
//!   cargo run --release --features dev --bin bench-scenes -- --project <dir> [--scenes <a,b>] [--size <W>x<H>] <out_dir>
//! ```
//!
//! Benches each scene (`--scenes` names them by file stem; every
//! `assets/scenes/*.scene` of the project by default) from its deterministic
//! viewpoints at 1920x1080 (or `--size`), keeps each report as
//! `<out_dir>/<project>/<scene>.json` and prints it with its change against the
//! scene's last run, then a one-table summary (`dev::bench::scenes`). Each scene
//! runs in a child process of this binary (`--scene <name>`), so its load is cold
//! and its peak memory its own. The project is read, never written.

use std::path::{absolute, Path, PathBuf};
use std::process::exit;
// lint: allow(subprocess) each scene benches in a child of this binary, so its load is cold and its memory peak its own
use std::process::Command;

use rusty::core::project::{take_flag, take_value, Access};
use rusty::core::settings::video::parse_size;
use rusty::dev::bench::scenes::{self, report, Options};

const USAGE: &str =
    "usage: bench-scenes --project <dir> [--scenes <a,b>] [--size <W>x<H>] <out_dir>";

fn main() {
    rusty::init();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    rusty::core::project::answer_help(&args, USAGE);
    let project = take_flag(&mut args).unwrap_or_else(|e| fail(&e));
    let list = take_value(&mut args, "--scenes").unwrap_or_else(|e| fail(&e));
    let one = take_value(&mut args, "--scene").unwrap_or_else(|e| fail(&e));
    let size = take_value(&mut args, "--size").unwrap_or_else(|e| fail(&e));
    let size = size
        .as_deref()
        .map(parse_size)
        .transpose()
        .unwrap_or_else(|e| fail(&e));
    let (Some(project), [out]) = (project, args.as_slice()) else {
        fail("bench-scenes needs --project and one out_dir")
    };
    let project = absolute(&project).unwrap_or(project);
    let name = project
        .file_name()
        .map_or("project".into(), |n| n.to_string_lossy().into_owned());
    let out = absolute(out)
        .unwrap_or_else(|_| PathBuf::from(out))
        .join(name);
    let opts = Options {
        size: size.unwrap_or(scenes::DEFAULT_SIZE),
        ..Options::default()
    };
    match one {
        Some(scene) => bench_one(&project, &scene, &out, opts),
        None => bench_all(&project, list.as_deref(), &out, opts),
    }
}

/// Bench `scene` in this process.
fn bench_one(project: &Path, scene: &str, out: &Path, opts: Options) {
    let opened = rusty::core::project::open(project, Access::Run);
    let opened = opened.unwrap_or_else(|e| fail(&format!("cannot open the project: {e}")));
    if let Err(e) = scenes::bench(&opened.root, scene, opened.settings, out, opts) {
        eprintln!("bench-scenes: {scene}: {e}");
        exit(1);
    }
}

/// Bench each scene in a child process, then print the summary of those that ran.
fn bench_all(project: &Path, list: Option<&str>, out: &Path, opts: Options) {
    let names: Vec<String> = match list {
        Some(list) => list
            .split(',')
            .map(|s| s.trim().trim_end_matches(".scene").to_string())
            .collect(),
        None => scenes::project_scenes(project),
    };
    if names.is_empty() {
        fail(&format!(
            "{} has no assets/scenes/*.scene",
            project.display()
        ));
    }
    let me = std::env::current_exe().unwrap_or_else(|e| fail(&e.to_string()));
    let (w, h) = opts.size;
    let mut failed = Vec::new();
    for scene in &names {
        let status = Command::new(&me)
            .args(["--project", &project.to_string_lossy(), "--scene", scene])
            .args(["--size", &format!("{w}x{h}")])
            .arg(out.parent().unwrap_or(out))
            .status();
        if !status.is_ok_and(|s| s.success()) {
            failed.push(scene.clone());
        }
    }
    let reports: Vec<_> = names
        .iter()
        .filter(|n| !failed.contains(n))
        .filter_map(|n| report::read(out, n))
        .collect();
    println!("{}", report::summary(&reports));
    if !failed.is_empty() {
        eprintln!("bench-scenes: failed: {}", failed.join(", "));
        exit(1);
    }
}

fn fail(e: &str) -> ! {
    eprintln!("bench-scenes: {e}\n{USAGE}");
    exit(2);
}
