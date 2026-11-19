//! src/bin/bench_scenes.rs — bench a game project's own scenes (dev-only, `make bench-scenes`).
//!
//! ```text
//! Usage:
//!   cargo run --release --features dev --bin bench-scenes -- --project <dir> [--scenes <a,b>]
//!       [--size <W>x<H>] [--runs <n>] [--before <dir>] [--profile] [--import] <out_dir>
//! ```
//!
//! Benches each scene (`--scenes` names them by their path under
//! `assets/scenes/` without the extension, `levels/a`; every `.scene` there,
//! subfolders included, by default) from its deterministic viewpoints at
//! 1920x1080 (or `--size`), `--runs` times (3 by default), keeps each report as
//! `<out_dir>/<project>-<hash>/<scene>.json` (the project folder's name and a
//! short hash of its canonical path, `scenes::out_key`) and prints it with its
//! change against the scene's last run, or against the kept report in
//! `--before`'s folder; then a one-table summary (`dev::bench::scenes`).
//!
//! Each scene runs in child processes of this binary (cold load, its own memory
//! peak): one benches it (`--scene`), and with `--profile` one more samples its
//! measured frames with the platform's profiler (`scenes::profile`), whose failure
//! leaves the timings standing (a refused profile is listed at the end); the
//! profiler is tried once before any scene, so a refusal ends the run. With
//! `--import`, once a run, one more imports a copy of the project without its
//! import cache and loads its first scene (`--import-out`, `scenes::import`),
//! handing both's time to that scene's run as `--import-ms`. The project is not written.

use std::ffi::OsString;
use std::path::{absolute, Path, PathBuf};
use std::process::exit;
// lint: allow(subprocess) each scene benches in children of this binary, so its load is cold and its memory peak its own
use std::process::Command;

use rusty::core::project::{take_flag, take_value, Access};
use rusty::core::settings::video::parse_size;
use rusty::dev::bench::scenes::{self, import, profile, report, Extra, Options};

const USAGE: &str = "usage: bench-scenes --project <dir> [--scenes <a,b>] [--size <W>x<H>] \
                     [--runs <n>] [--before <dir>] [--profile] [--import] <out_dir>";

/// What one invocation was asked for.
struct Run {
    project: PathBuf,
    /// The project's reports folder under the out dir.
    out: PathBuf,
    opts: Options,
    before: Option<PathBuf>,
    profile: bool,
    /// `--import`: measure `import_ms` once this run.
    import: bool,
}

fn main() {
    rusty::init();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    rusty::core::project::answer_help(&args, USAGE);
    let value = |args: &mut Vec<String>, flag| take_value(args, flag).unwrap_or_else(|e| fail(&e));
    let project = take_flag(&mut args).unwrap_or_else(|e| fail(&e));
    let list = value(&mut args, "--scenes");
    let one = value(&mut args, "--scene");
    let size = value(&mut args, "--size").map(|s| parse_size(&s).unwrap_or_else(|e| fail(&e)));
    let runs = value(&mut args, "--runs").map(|r| match r.parse::<u32>() {
        Ok(n) if n > 0 => n,
        _ => fail(&format!("--runs wants a whole number above 0, not {r}")),
    });
    let before = value(&mut args, "--before").map(PathBuf::from);
    let import_out = value(&mut args, "--import-out").map(PathBuf::from);
    let import_ms = value(&mut args, "--import-ms").and_then(|v| v.parse::<f64>().ok());
    let profile = take_switch(&mut args, "--profile");
    let import = take_switch(&mut args, "--import");
    let (Some(project), [out]) = (project, args.as_slice()) else {
        fail("bench-scenes needs --project and one out_dir")
    };
    let project = std::fs::canonicalize(&project)
        .unwrap_or_else(|e| fail(&format!("{}: {e}", project.display())));
    let key = scenes::out_key(&project);
    let out = absolute(out)
        .unwrap_or_else(|_| PathBuf::from(out))
        .join(&key);
    let opts = Options {
        size: size.unwrap_or(scenes::DEFAULT_SIZE),
        runs: runs.unwrap_or(scenes::RUNS),
        ..Options::default()
    };
    let before = before.map(|b| absolute(&b).unwrap_or(b));
    let run = Run {
        project,
        out,
        opts,
        before,
        profile,
        import,
    };
    match (one, import_out) {
        (Some(scene), Some(file)) => import_one(&run, &scene, &file),
        (Some(scene), None) if run.profile => profile_one(&run, &scene),
        (Some(scene), None) => bench_one(&run, &scene, key, import_ms),
        (None, _) => bench_all(&run, list.as_deref()),
    }
}

/// Bench `scene` in this process.
fn bench_one(run: &Run, scene: &str, key: String, import_ms: Option<f64>) {
    let opened = rusty::core::project::open(&run.project, Access::Run);
    let opened = opened.unwrap_or_else(|e| fail(&format!("cannot open the project: {e}")));
    let extra = Extra {
        baseline: run.before.clone(),
        key,
        import_ms,
    };
    let done = scenes::bench(
        &opened.root,
        scene,
        opened.settings,
        &run.out,
        run.opts,
        &extra,
    );
    if let Err(e) = done {
        eprintln!("bench-scenes: {scene}: {e}");
        exit(1);
    }
}

/// Import the project (a cache-cleared copy), load `scene`, and write both's ms to `file`.
fn import_one(run: &Run, scene: &str, file: &Path) {
    let opened = rusty::core::project::open(&run.project, Access::Run);
    let opened = opened.unwrap_or_else(|e| fail(&format!("cannot open the copy: {e}")));
    let scratch = file.parent().unwrap_or(Path::new("."));
    eprintln!("{}", rusty::asset::import::notice(&opened.root));
    let imported = import::import().unwrap_or_else(|e| fail(&format!("import failed: {e}")));
    let loaded = scenes::load(scratch, &opened.root, scene, opened.settings, run.opts);
    let ms = loaded.map_or_else(
        |e| fail(&format!("{scene}: cold load failed: {e}")),
        |l| imported + l.1,
    );
    // lint: allow(asset-write) a number handed to the parent process, deleted after
    let wrote = std::fs::write(file, ms.to_string());
    wrote.unwrap_or_else(|e| fail(&e.to_string()));
}

/// Profile `scene` in this process.
fn profile_one(run: &Run, scene: &str) {
    let profiler = profile::find(std::env::var_os("PATH").as_deref()).unwrap_or_else(|e| fail(&e));
    let opened = rusty::core::project::open(&run.project, Access::Run);
    let opened = opened.unwrap_or_else(|e| fail(&format!("cannot open the project: {e}")));
    let at = (opened.root.as_path(), scene);
    if let Err(e) = profile::run(&profiler, at, opened.settings, &run.out, run.opts) {
        eprintln!("bench-scenes: {scene}: profile: {e}");
        exit(1);
    }
}

/// Bench each scene in child processes, then print the summary of those that ran.
fn bench_all(run: &Run, list: Option<&str>) {
    if run.profile {
        // Before anything is benched: a missing or refused profiler fails at once.
        let profiler = profile::find(std::env::var_os("PATH").as_deref());
        let profiler = profiler.unwrap_or_else(|e| fail(&e));
        let probed = profile::probe(&profiler, &run.out.join(".profile-probe"));
        probed.unwrap_or_else(|e| refused(&e));
    }
    let names: Vec<String> = match list {
        Some(list) => list
            .split(',')
            .map(|s| s.trim().trim_end_matches(".scene").to_string())
            .collect(),
        None => scenes::project_scenes(&run.project),
    };
    let Some(first) = names.first() else {
        fail(&format!(
            "{} has no .scene under assets/scenes/",
            run.project.display()
        ))
    };
    // The import is the project's, not a scene's: once a run, on its first scene.
    let import_ms = run.import.then(|| cold_import(run, first)).flatten();
    let (mut failed, mut unprofiled) = (Vec::new(), Vec::new());
    for (n, scene) in names.iter().enumerate() {
        match bench_scene(run, scene, import_ms.filter(|_| n == 0)) {
            Benched::Failed => failed.push(scene.clone()),
            Benched::Unprofiled => unprofiled.push(scene.clone()),
            Benched::Done => {}
        }
    }
    let reports: Vec<_> = names
        .iter()
        .filter(|n| !failed.contains(n))
        .filter_map(|n| report::read(&run.out, n))
        .collect();
    println!("{}", report::summary(&reports));
    if !unprofiled.is_empty() {
        let scenes = unprofiled.join(", ");
        eprintln!("bench-scenes: no profile (their timings stand): {scenes}");
    }
    let fresh = |n: &&String| run.profile && !failed.contains(n) && !unprofiled.contains(n);
    if let Some(said) = profile::refusals(&run.out, names.iter().filter(fresh)) {
        eprintln!("bench-scenes: {said}");
    }
    if !failed.is_empty() {
        eprintln!("bench-scenes: failed: {}", failed.join(", "));
    }
    if !failed.is_empty() || !unprofiled.is_empty() {
        exit(1);
    }
}

/// How one scene's children went.
enum Benched {
    Done,
    /// The timed run failed: the scene has no report this run.
    Failed,
    /// The timed run stands; its profile failed.
    Unprofiled,
}

/// Bench `scene` in its children: the timed run (given the run's `import_ms`),
/// then the profile when asked for, whose failure leaves the timings standing.
fn bench_scene(run: &Run, scene: &str, import_ms: Option<f64>) -> Benched {
    let mut timed = child(run, &run.project, scene);
    if let Some(ms) = import_ms {
        timed.args(["--import-ms", &ms.to_string()]);
    }
    if let Some(before) = &run.before {
        timed.arg("--before").arg(before);
    }
    if !timed.status().is_ok_and(|s| s.success()) {
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

/// `import_ms`: `scene`'s load in a cache-less copy of the project, in a child;
/// `None` (said) when the copy or the load failed, and the bench goes on.
fn cold_import(run: &Run, scene: &str) -> Option<f64> {
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
    let root: OsString = run.out.parent().unwrap_or(&run.out).into();
    cmd.arg(root);
    cmd
}

/// Remove `flag` from `args`, saying whether it was there.
fn take_switch(args: &mut Vec<String>, flag: &str) -> bool {
    let at = args.iter().position(|a| a == flag);
    at.map(|i| args.remove(i)).is_some()
}

/// The profiler would not record here: what it said, and what to set; no usage.
fn refused(e: &str) -> ! {
    eprintln!("bench-scenes: {e}");
    exit(2);
}

fn fail(e: &str) -> ! {
    eprintln!("bench-scenes: {e}\n{USAGE}");
    exit(2);
}
