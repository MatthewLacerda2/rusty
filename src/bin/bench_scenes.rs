//! src/bin/bench_scenes.rs — bench a game project's own scenes (dev-only, `make bench-scenes`).
//!
//! ```text
//! Usage:
//!   cargo run --release --features dev --bin bench-scenes -- --project <dir> [--scenes <a,b>]
//!       [--size <W>x<H>] [--runs <n>] [--load-runs <n>] [--before <dir>] [--set <k=v>]...
//!       [--profile] [--import] [--script] [--no-editor]
//!       <out_dir>  |  --project <dir> --pending-imports
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
//! Each scene runs in child processes of this binary (its own memory peak): first
//! `--load-runs` less one (3 loads in all by default) only load it (`--load-out`,
//! the first `load_cold_ms`), each in a fresh process, then one benches it
//! (`--scene`, handed those loads with `--loads`, its own load the last of them:
//! `scenes::load`), and with `--profile` one more samples its
//! measured frames with the platform's profiler (`scenes::profile`), whose failure
//! leaves the timings standing (a refused profile is listed at the end); the
//! profiler is tried once before any scene, so a refusal ends the run. With
//! `--import`, once a run, one more imports a copy of the project without its
//! import cache and loads its first scene (`--import-out`, `scenes::import`),
//! handing both's time to that scene's run as `--import-ms`. `--set` overrides the
//! running quality preset in every child (`scenes::set`). `--script` times the
//! scripts' engine verbs in the timed child (`bench::verbs`), and every child of
//! the run writes under `<out_dir>/<project>-<hash>/script/`, so a plain run's
//! history is never compared with one and a `--profile` beside it (played without
//! the timers) is kept and checked there; `--pending-imports` lists
//! the images an import would encode, exiting 1 if any. The project is not written.

use std::path::{absolute, Path, PathBuf};
use std::process::exit;

#[path = "bench_scenes/children.rs"]
mod children;
use children::{bench_scene, cold_import, Benched};
#[path = "bench_scenes/solo.rs"]
mod solo;
use solo::{editor_one, import_one, load_one, profile_one};

use rusty::core::project::Opened;
use rusty::dev::bench::scenes::{self, import, profile, report, set::Overrides, Extra, Options};

const USAGE: &str = "usage: bench-scenes --project <dir> [--scenes <a,b>] [--size <W>x<H>] [--runs <n>] \
     [--load-runs <n>] [--before <dir>] [--set <k=v>]... [--profile] [--import] [--script] [--no-editor] \
     <out_dir> | --project <dir> --pending-imports";

/// What one invocation was asked for.
struct Run {
    project: PathBuf,
    /// The out dir as given, absolute: what each child is handed.
    root: PathBuf,
    /// The project's reports folder under it ([`reports_dir`]).
    out: PathBuf,
    opts: Options,
    /// How many times each scene is loaded, each in a process of its own.
    load_runs: u32,
    before: Option<PathBuf>,
    profile: bool,
    /// `--import`: measure `import_ms` once this run.
    import: bool,
    /// `--script`: time the scripts' engine verbs in the timed child; every
    /// child writes under the `script/` folder.
    script: bool,
    /// Run the editor round on each scene (off with `--no-editor`).
    editor: bool,
    set: Overrides,
}

fn main() {
    rusty::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    rusty::core::project::answer_help(&args, USAGE);
    let a = scenes::args::parse(args).unwrap_or_else(|e| fail(&e));
    let Some(project) = a.project else {
        fail("bench-scenes needs --project")
    };
    let project = std::fs::canonicalize(&project)
        .unwrap_or_else(|e| fail(&format!("{}: {e}", project.display())));
    if a.pending_imports {
        let found = import::pending(&project).unwrap_or_else(|e| fail(&e));
        found
            .iter()
            .for_each(|image| println!("{}", image.display()));
        exit(i32::from(!found.is_empty()));
    }
    let [out] = a.rest.as_slice() else {
        fail("bench-scenes needs one out_dir")
    };
    let key = scenes::out_key(&project);
    let root = absolute(out).unwrap_or_else(|_| PathBuf::from(out));
    let out = reports_dir(&root, &key, a.script);
    let opts = Options {
        size: a.size.unwrap_or(scenes::DEFAULT_SIZE),
        runs: a.runs.unwrap_or(scenes::RUNS),
        ..Options::default()
    };
    let run = Run {
        project,
        root,
        out,
        opts,
        load_runs: a.load_runs.unwrap_or(scenes::LOAD_RUNS),
        before: a.before.map(|b| absolute(&b).unwrap_or(b)),
        profile: a.profile,
        import: a.import,
        script: a.script,
        editor: !a.no_editor,
        set: a.set,
    };
    if let (Some(scene), Some(file)) = (&a.one, &a.editor_out) {
        return editor_one(&run, scene, file);
    }
    match (a.one, a.import_out) {
        (Some(scene), Some(file)) => import_one(&run, &scene, &file),
        (Some(scene), None) if run.profile => profile_one(&run, &scene),
        (Some(scene), None) => match a.load_out {
            Some(file) => load_one(&run, &scene, &file),
            None => {
                let kept = Kept {
                    import_ms: a.import_ms,
                    loads: a.loads,
                    editor: a.editor,
                };
                bench_one(&run, &scene, key, kept);
            }
        },
        (None, _) => bench_all(&run, a.list.as_deref()),
    }
}

/// The project's reports folder under the out dir `root`: `<root>/<key>`, and
/// its `script/` for a `--script` run, so a plain run's history never meets one.
fn reports_dir(root: &Path, key: &str, script: bool) -> PathBuf {
    let dir = root.join(key);
    if script {
        dir.join("script")
    } else {
        dir
    }
}

/// The project at `path` opened to run, the run's `--set` over its running preset.
fn open(run: &Run, path: &Path, what: &str) -> Opened {
    scenes::open(path, &run.set).unwrap_or_else(|e| fail(&format!("cannot open {what}: {e}")))
}

/// What the scene's earlier children handed its bench.
struct Kept {
    import_ms: Option<f64>,
    /// The loads' folder (`--loads`).
    loads: Option<PathBuf>,
    /// The editor round's file (`--editor`).
    editor: Option<PathBuf>,
}

/// Bench `scene` in this process, after the loads and the editor round `kept` names.
fn bench_one(run: &Run, scene: &str, key: String, kept: Kept) {
    if run.script {
        // Before the first VM: one started earlier keeps the surface it has.
        rusty::dev::stats::verbs::arm();
    }
    let opened = open(run, &run.project, "the project");
    let loads = kept
        .loads
        .as_deref()
        .map(scenes::load::read_all)
        .transpose();
    let editor = kept.editor.as_deref().map(scenes::editor::read).transpose();
    let extra = Extra {
        baseline: run.before.clone(),
        key,
        import_ms: kept.import_ms,
        loads: loads.unwrap_or_else(|e| fail(&e)).unwrap_or_default(),
        set: run.set.said.clone(),
        editor: editor.unwrap_or_else(|e| fail(&e)),
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

/// The profiler would not record here: what it said, and what to set; no usage.
fn refused(e: &str) -> ! {
    eprintln!("bench-scenes: {e}");
    exit(2);
}

fn fail(e: &str) -> ! {
    eprintln!("bench-scenes: {e}\n{USAGE}");
    exit(2);
}
