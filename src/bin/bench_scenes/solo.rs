//! What `bench-scenes`' one-job children do (`src/bin/bench_scenes.rs`): load a
//! scene and keep the load, run the editor round on it, import a cache-cleared
//! copy, or profile it, each in this process, its result kept for the parent.

use std::path::Path;
use std::process::exit;

use rusty::dev::bench::scenes::{self, import, profile};

use super::{fail, open, Run};

/// Load `scene` in this process and keep the load at `file`.
pub fn load_one(run: &Run, scene: &str, file: &Path) {
    let opened = open(run, &run.project, "the project");
    let scratch = file.parent().unwrap_or(Path::new("."));
    let loaded = scenes::load(scratch, &opened.root, scene, opened.settings, run.opts);
    let (_, load) = loaded.unwrap_or_else(|e| fail(&format!("{scene}: load failed: {e}")));
    scenes::load::write(file, &load).unwrap_or_else(|e| fail(&e));
}

/// Run the editor round on `scene` in this process and keep it at `file`.
pub fn editor_one(run: &Run, scene: &str, file: &Path) {
    use rusty::shell::editor::capture::bench;
    let opened = open(run, &run.project, "the project");
    let path = opened.root.join(format!("assets/scenes/{scene}.scene"));
    let world = bench::editor_world(&path.to_string_lossy(), opened.settings);
    let mut game = world.unwrap_or_else(|e| fail(&format!("{scene}: editor round: {e}")));
    let opts = bench::Round {
        size: run.opts.size,
        warmup: run.opts.warmup,
        frames: run.opts.frames,
        runs: run.opts.runs,
        warm: run.opts.warm,
        save_dir: file.with_extension("save"),
    };
    let round = bench::round(&mut game, &opts);
    match round.unwrap_or_else(|e| fail(&format!("{scene}: editor round: {e}"))) {
        Some(round) => scenes::editor::write(file, &round).unwrap_or_else(|e| fail(&e)),
        None => eprintln!("bench-scenes: {scene}: no adapter: no editor round"),
    }
}

/// Import the project (a cache-cleared copy), load `scene`, and write both's ms to `file`.
pub fn import_one(run: &Run, scene: &str, file: &Path) {
    let opened = open(run, &run.project, "the copy");
    let scratch = file.parent().unwrap_or(Path::new("."));
    eprintln!("{}", rusty::asset::import::notice(&opened.root));
    let imported = import::import().unwrap_or_else(|e| fail(&format!("import failed: {e}")));
    let loaded = scenes::load(scratch, &opened.root, scene, opened.settings, run.opts);
    let ms = loaded.map_or_else(
        |e| fail(&format!("{scene}: cold load failed: {e}")),
        |l| imported + l.1.total_ms(),
    );
    // lint: allow(asset-write) a number handed to the parent process, deleted after
    let wrote = std::fs::write(file, ms.to_string());
    wrote.unwrap_or_else(|e| fail(&e.to_string()));
}

/// Profile `scene` in this process.
pub fn profile_one(run: &Run, scene: &str) {
    let profiler = profile::find(std::env::var_os("PATH").as_deref()).unwrap_or_else(|e| fail(&e));
    let opened = open(run, &run.project, "the project");
    let at = (opened.root.as_path(), scene);
    if let Err(e) = profile::run(&profiler, at, opened.settings, &run.out, run.opts) {
        eprintln!("bench-scenes: {scene}: profile: {e}");
        exit(1);
    }
}
