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
    load_runs: u32,
    before: Option<PathBuf>,
    profile: bool,
    /// `--import`: measure `import_ms` once this run.
    import: bool,
    script: bool,
    /// Run the editor round on each scene (off with `--no-editor`).
    editor: bool,
    set: Overrides,
}

fn main() {
    rusty::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    rusty::core::project::answer_help(&args, USAGE);
    let mut a = scenes::args::parse(args).unwrap_or_else(|e| fail(&e));
    let Some(project) = a.project.take() else {
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
        switches: a.set.switches(),
        ..Options::default()
    };
    let run = Run {
        project,
        root,
        out,
        opts,
        load_runs: a.load_runs.unwrap_or(scenes::LOAD_RUNS),
        before: a.before.take().map(|b| absolute(&b).unwrap_or(b)),
        profile: a.profile,
        import: a.import,
        script: a.script,
        editor: !a.no_editor,
        set: std::mem::take(&mut a.set),
    };
    dispatch(&run, a, key);
}

fn dispatch(run: &Run, a: scenes::args::Args, key: String) {
    if let (Some(scene), Some(file)) = (&a.one, &a.editor_out) {
        return editor_one(run, scene, file);
    }
    match (a.one, a.import_out) {
        (Some(scene), Some(file)) => import_one(run, &scene, &file),
        (Some(scene), None) if run.profile => profile_one(run, &scene),
        (Some(scene), None) => match a.load_out {
            Some(file) => load_one(run, &scene, &file),
            None => {
                let kept = Kept {
                    import_ms: a.import_ms,
                    loads: a.loads,
                    editor: a.editor,
                };
                bench_one(run, &scene, key, kept);
            }
        },
        (None, _) => bench_all(run, a.list.as_deref()),
    }
}

fn reports_dir(root: &Path, key: &str, script: bool) -> PathBuf {
    let dir = root.join(key);
    if script {
        dir.join("script")
    } else {
        dir
    }
}

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

fn bench_one(run: &Run, scene: &str, key: String, kept: Kept) {
    if run.script {
        rusty::dev::stats::verbs::arm();
    }
    let opened = open(run, &run.project, "the project");
    let loads = kept
        .loads
        .as_deref()
        .map(scenes::load::read_all)
        .transpose();
    let editor = kept.editor.as_deref().and_then(|file| {
        let read = scenes::editor::read(file);
        read.map_err(|e| eprintln!("bench-scenes: {scene}: no editor rows: {e}"))
            .ok()
    });
    let extra = Extra {
        baseline: run.before.clone(),
        key,
        import_ms: kept.import_ms,
        loads: loads.unwrap_or_else(|e| fail(&e)).unwrap_or_default(),
        set: run.set.said.clone(),
        editor,
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

fn bench_all(run: &Run, list: Option<&str>) {
    if run.profile {
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
