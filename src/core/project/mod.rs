//! src/core/project/ — the open game project (#829).
//!
//! A game lives in a **project folder** anywhere on disk, in its own repo — Unity's
//! model, where the Hub or `-projectPath` opens a folder and the editor's working
//! directory *is* that folder. rusty does the same: [`open`] creates the skeleton
//! and makes the project root the process's
//! working directory. Every path a project stores (scene asset refs, script paths,
//! build settings) is relative to that root, `/`-separated, so it names the same file
//! on every machine and the engine code that reads it stays a plain relative read.
//!
//! The skeleton is Unity's and Unreal's shape:
//!
//! ```text
//! <project>/
//!   assets/              the game: scenes, scripts, prefabs, models, textures,
//!                        audio, materials, authored shaders (Unity's Assets/)
//!   project.rusty        marks the folder as a project: the engine commit it was
//!                        last opened with, and the build settings (#853)
//!   .seeded              the seed manifest (#746)
//!   cache/               anything the engine can regenerate (Unity's Library/)
//!   saved/               the player's save data (Unreal's Saved/)
//! ```
//!
//! `cache/` and `saved/` each hold a `.gitignore` of `*`, so they stay out of git
//! without the engine writing anything at the root: version control is the user's.
//!
//! The engine's own content (shaders, bundled scripts, the preview model) is not the
//! project's: it is found by [`engine_dir`], wherever the project is.

pub mod bench;
mod commit;
mod file;
mod locate;

use std::path::{Path, PathBuf};

pub use commit::{engine_commit, UNKNOWN_COMMIT};
pub use file::{EngineCheck, ProjectFile, PROJECT_FILE};
pub use locate::{engine_dir, engine_path, locate};

/// The project's content folder, relative to its root (Unity's `Assets/`).
pub const ASSETS_DIR: &str = "assets";
/// Regenerable engine output (Unity's `Library/`, Unreal's `Intermediate/`).
pub const CACHE_DIR: &str = "cache";
/// The player's save data (Unreal's `Saved/`).
pub const SAVED_DIR: &str = "saved";
/// The project a binary opens when none is named: `./project`, the folder the
/// engine used before projects could live anywhere.
pub const DEFAULT_PROJECT_DIR: &str = "project";
/// The command-line flag every binary takes to name its project.
const PROJECT_FLAG: &str = "--project";

/// Where a project keeps its Lua scripts, relative to the root.
pub const SCRIPTS_DIR: &str = "assets/scripts";
/// Where a project keeps its textures (baked material maps included).
pub const TEXTURES_DIR: &str = "assets/textures";
/// Where a project keeps its generated and authored shaders.
pub const SHADERS_DIR: &str = "assets/shaders";
/// Where generated assets live: provider output (#384) and converted model sources
/// (#1024). Whether a game's repo versions these files is still open (#866), so
/// every writer names this one folder and follows whatever is decided for it.
pub const GENERATED_DIR: &str = "assets/generated";

/// The folders [`create_skeleton`] makes, relative to the root.
pub const SKELETON: &[&str] = &[
    "assets/scenes",
    SCRIPTS_DIR,
    "assets/prefabs",
    "assets/models",
    TEXTURES_DIR,
    "assets/audio",
    "assets/materials",
    SHADERS_DIR,
    CACHE_DIR,
    SAVED_DIR,
];

/// Create the project skeleton under `dir` (and `dir` itself). Idempotent: what
/// exists is left alone.
pub fn create_skeleton(dir: &Path) -> Result<(), String> {
    for sub in SKELETON {
        let path = dir.join(sub);
        std::fs::create_dir_all(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    for ignored in [CACHE_DIR, SAVED_DIR] {
        let path = dir.join(ignored).join(".gitignore");
        if !path.exists() {
            // lint: allow(asset-write) project settings, not read by a path-keyed cache
            std::fs::write(&path, "*\n").map_err(|e| format!("{}: {e}", path.display()))?;
        }
    }
    Ok(())
}

/// How a binary opens its project: whether it may bring `project.rusty` up to date.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// The editor and the agent sessions: create a missing `project.rusty` and record
    /// the running engine's commit.
    Edit,
    /// The player, scenario runs and captures: check the engine, write nothing. A
    /// shipped game never rewrites its project, and a run over the tracked fixture
    /// project leaves the checkout clean.
    Run,
}

/// What [`open`] found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Opened {
    /// The project root, absolute; now the working directory.
    pub root: PathBuf,
    /// The engine the project was last opened with against the running one. Opening
    /// prints its warning on stderr; a frontend with a UI also shows it.
    pub engine: EngineCheck,
}

/// Open the project at `dir`: create its skeleton, check
/// (and with [`Access::Edit`] record) the engine in `project.rusty`, and make it the
/// working directory. Call it once, at startup, before anything reads a project path
/// — and resolve any other relative path the command line named first
/// ([`std::path::absolute`]), since the working directory moves.
///
/// Progress goes to stderr: `session-mcp`'s stdout carries only JSON-RPC.
pub fn open(dir: &Path, access: Access) -> Result<Opened, String> {
    let (opened, mut lines) = prepare(dir, access)?;
    let root = &opened.root;
    // lint: allow(statement-opt-out) the working-directory move, not a clock read
    #[expect(
        clippy::disallowed_methods,
        reason = "opening a project makes it the working directory, once, at startup"
    )]
    std::env::set_current_dir(root).map_err(|e| format!("{}: {e}", root.display()))?;
    lines.push(format!("Opened {}", root.display()));
    lines.extend(opened.engine.warning());
    for line in lines {
        eprintln!("[Project] {line}");
    }
    Ok(opened)
}

/// Everything [`open`] does but move the working directory: make the folder a project
/// and check its engine. Returns what it found and the progress lines to report.
pub(crate) fn prepare(dir: &Path, access: Access) -> Result<(Opened, Vec<String>), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let root =
        crate::core::paths::canonicalize(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    create_skeleton(&root)?;
    let (engine, lines) = match access {
        Access::Edit => file::record(&root)?,
        Access::Run => (file::inspect(&root)?, Vec::new()),
    };
    Ok((Opened { root, engine }, lines))
}

/// The engine check of the project at `dir`, without opening it or writing anything:
/// what a picker shows before it opens a project last opened by another engine.
pub fn inspect(dir: &Path) -> Result<EngineCheck, String> {
    file::inspect(dir)
}

/// Whether `dir` already holds a project: its [`PROJECT_FILE`], or an `assets/`
/// folder (a project only ever opened to run has no file). A picker opens only these, so a stray folder
/// is never filled with a skeleton by accident.
pub fn is_project(dir: &Path) -> bool {
    dir.join(PROJECT_FILE).is_file() || dir.join(ASSETS_DIR).is_dir()
}

/// The open project's root: the working directory, which [`open`] made the root.
/// (The in-process tests never open one; for them it is wherever they run.)
pub fn root() -> PathBuf {
    std::env::current_dir().unwrap_or_default()
}

/// Remove `--project <dir>` (or `--project=<dir>`) from `args` and return the
/// directory. `Err` when the flag has no value.
pub fn take_flag(args: &mut Vec<String>) -> Result<Option<PathBuf>, String> {
    let Some(at) = args
        .iter()
        .position(|a| a == PROJECT_FLAG || a.starts_with("--project="))
    else {
        return Ok(None);
    };
    let flag = args.remove(at);
    if let Some(dir) = flag.strip_prefix("--project=") {
        return Ok(Some(PathBuf::from(dir)));
    }
    if at >= args.len() || args[at].starts_with("--") {
        return Err(format!("{PROJECT_FLAG} needs a directory"));
    }
    Ok(Some(PathBuf::from(args.remove(at))))
}

/// What every binary does first: take `--project` from `args`, find the project
/// ([`locate`]) and [`open`] it. Exits with status 2 and one line on stderr when the
/// flag is malformed or the folder can't be opened — a binary can't run without it.
pub fn open_from_args(args: &mut Vec<String>, access: Access) -> Opened {
    let opened = take_flag(args).and_then(|named| open(&locate(named), access));
    opened.unwrap_or_else(|e| {
        eprintln!("[Project] cannot open the project: {e}");
        std::process::exit(2);
    })
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
