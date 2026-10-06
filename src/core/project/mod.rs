//! src/core/project/ — the open game project (#829).
//!
//! A game lives in a **project folder** anywhere on disk, in its own repo — Unity's
//! model, where the Hub or `-projectPath` opens a folder and the editor's working
//! directory *is* that folder. rusty does the same: [`open`] creates the skeleton,
//! carries a legacy `./project` forward, and makes the project root the process's
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

mod file;
mod locate;
mod migrate;

use std::path::{Path, PathBuf};

pub use file::{
    EngineCheck, ProjectFile, ENGINE_COMMIT, LEGACY_BUILD_SETTINGS, PROJECT_FILE, UNKNOWN_COMMIT,
};
pub use locate::{engine_dir, engine_path, locate, packaged_project};

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
pub const PROJECT_FLAG: &str = "--project";

/// The folders [`create_skeleton`] makes, relative to the root.
pub const SKELETON: &[&str] = &[
    "assets/scenes",
    "assets/scripts",
    "assets/prefabs",
    "assets/models",
    "assets/textures",
    "assets/audio",
    "assets/materials",
    "assets/shaders",
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
            std::fs::write(&path, "*\n").map_err(|e| format!("{}: {e}", path.display()))?;
        }
    }
    Ok(())
}

/// How a binary opens its project: whether it may bring `project.rusty` up to date.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// The editor and the agent sessions: create a missing `project.rusty` (folding a
    /// legacy `build_settings.json` in) and record the running engine's commit.
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

/// Open the project at `dir`: create its skeleton, migrate a legacy layout, check
/// (and with [`Access::Edit`] record) the engine in `project.rusty`, and make it the
/// working directory. Call it once, at startup, before anything reads a project path
/// — and resolve any other relative path the command line named first
/// ([`std::path::absolute`]), since the working directory moves.
///
/// Progress goes to stderr: `session-mcp`'s stdout carries only JSON-RPC.
pub fn open(dir: &Path, access: Access) -> Result<Opened, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let root = std::fs::canonicalize(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut lines = migrate::legacy_layout(&root)?;
    create_skeleton(&root)?;
    let engine = match access {
        Access::Edit => {
            let (engine, recorded) = file::record(&root)?;
            lines.extend(recorded);
            engine
        }
        Access::Run => file::inspect(&root)?,
    };
    std::env::set_current_dir(&root).map_err(|e| format!("{}: {e}", root.display()))?;
    lines.push(format!("Opened {}", root.display()));
    lines.extend(engine.warning());
    for line in lines {
        eprintln!("[Project] {line}");
    }
    Ok(Opened { root, engine })
}

/// The engine check of the project at `dir`, without opening it or writing anything:
/// what a picker shows before it opens a project last opened by another engine.
pub fn inspect(dir: &Path) -> Result<EngineCheck, String> {
    file::inspect(dir)
}

/// Whether `dir` already holds a project: its [`PROJECT_FILE`], or (from before the
/// file existed) an `assets/` folder. A picker opens only these, so a stray folder
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
