use std::ffi::OsString;
use std::path::Path;
// lint: allow(subprocess) each scene benches in children of this binary, so each load is a fresh process's and its memory peak its own
use std::process::Command;

use rusty::dev::bench::scenes::import;

use super::{fail, Run};

/// How one scene's children went.
pub enum Benched {
    Done,
    /// The timed run failed: the scene has no report this run.
    Failed,
    /// The timed run stands; its profile failed.
    Unprofiled,
}

pub fn bench_scene(run: &Run, scene: &str, import_ms: Option<f64>) -> Benched {
    let loads = run.out.join(".loads");
    let _ = std::fs::remove_dir_all(&loads);
    if !earlier_loads(run, scene, &loads) {
        let _ = std::fs::remove_dir_all(&loads);
        return Benched::Failed;
    }
    let editor = run
        .editor
        .then(|| editor_round(run, scene, &loads))
        .flatten();
    let mut timed = child(run, &run.project, scene);
    timed.arg("--loads").arg(&loads);
    if let Some(file) = &editor {
        timed.arg("--editor").arg(file);
    }
    if let Some(ms) = import_ms {
        timed.args(["--import-ms", &ms.to_string()]);
    }
    if let Some(before) = &run.before {
        timed.arg("--before").arg(before);
    }
    let benched = timed.status().is_ok_and(|s| s.success());
    let _ = std::fs::remove_dir_all(&loads);
    if !benched {
        return Benched::Failed;
    }
    if !run.profile {
        return Benched::Done;
    }
    let mut profiled = child(run, &run.project, scene);
    let status = profiled.arg("--profile").status();
    if status.is_ok_and(|s| s.success()) {
        Benched::Done
    } else {
        eprintln!("bench-scenes: {scene}: the profile failed; its timings stand");
        Benched::Unprofiled
    }
}

fn editor_round(run: &Run, scene: &str, dir: &Path) -> Option<std::path::PathBuf> {
    let file = dir.join("editor.json");
    let mut round = child(run, &run.project, scene);
    let status = round.arg("--editor-out").arg(&file).status();
    if !status.is_ok_and(|s| s.success()) {
        eprintln!(
            "bench-scenes: {scene}: the editor round failed; the scene is benched without it"
        );
        return None;
    }
    file.exists().then_some(file)
}

fn earlier_loads(run: &Run, scene: &str, dir: &Path) -> bool {
    if let Err(e) = std::fs::create_dir_all(dir) {
        eprintln!("bench-scenes: {}: {e}", dir.display());
        return false;
    }
    for n in 0..run.load_runs - 1 {
        let mut load = child(run, &run.project, scene);
        let status = load
            .arg("--load-out")
            .arg(dir.join(format!("{n}.json")))
            .status();
        if !status.is_ok_and(|s| s.success()) {
            eprintln!(
                "bench-scenes: {scene}: load {} of {} failed",
                n + 1,
                run.load_runs
            );
            return false;
        }
    }
    true
}

pub fn cold_import(run: &Run, scene: &str) -> Option<f64> {
    let copy = run.out.join(".import-copy");
    let file = copy.join(".import_ms");
    let loaded = import::scratch(&run.project, &copy).map(|()| {
        let mut cold = child(run, &copy, scene);
        cold.arg("--import-out").arg(&file).status()
    });
    let ms = match loaded {
        Ok(Ok(s)) if s.success() => std::fs::read_to_string(&file).ok(),
        Ok(_) => None,
        Err(e) => {
            eprintln!("bench-scenes: cannot copy the project, no import_ms: {e}");
            let _ = std::fs::remove_dir_all(&copy);
            return None;
        }
    };
    let _ = std::fs::remove_dir_all(&copy);
    let ms = ms.and_then(|t| t.trim().parse().ok());
    if ms.is_none() {
        eprintln!("bench-scenes: {scene}: the cold load failed: no import_ms");
    }
    ms
}

/// This binary on `scene` of the project at `project`, with the run's size and runs.
fn child(run: &Run, project: &Path, scene: &str) -> Command {
    let me = std::env::current_exe().unwrap_or_else(|e| fail(&e.to_string()));
    let (w, h) = run.opts.size;
    let mut cmd = Command::new(me);
    cmd.arg("--project").arg(project).args(["--scene", scene]);
    cmd.args([
        "--size",
        &format!("{w}x{h}"),
        "--runs",
        &run.opts.runs.to_string(),
    ]);
    for pair in &run.set.said {
        cmd.args(["--set", pair]);
    }
    // Every child of a `--script` run writes where the run reads, `script/`;
    // only the timed one (`bench_one`) arms the timers.
    if run.script {
        cmd.arg("--script");
    }
    let root: OsString = run.root.clone().into();
    cmd.arg(root);
    cmd
}
