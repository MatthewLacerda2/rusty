mod agent;
mod args;
pub mod bench;
mod boot;
mod content;
mod engine;
mod file;
mod locate;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub use agent::{CLAUDE_FILE, MCP_FILE};
pub use args::{answer_help, take_flag, take_value};
pub use boot::{boot_scene, remember_scene, scene_to_remember, BootPlan, LAST_SCENE_PATH};
pub use content::{engine_relative, ContentRoots, ENGINE_PREFIX};
#[doc(hidden)]
pub use engine::commit::pin as pin_engine;
pub use engine::commit::{engine_commit, engine_order, UNKNOWN_COMMIT};
pub use engine::content::{self as engine_content, EngineStamp};
pub use engine::{EngineCheck, Verdict};
pub use file::{ProjectFile, ProjectSettings, PROJECT_FILE};
pub use locate::{checkout, engine_dir, engine_path, locate};

pub const ASSETS_DIR: &str = "assets";
/// Regenerable engine output (Unity's `Library/`, Unreal's `Intermediate/`).
pub const CACHE_DIR: &str = "cache";
pub const SAVED_DIR: &str = "saved";
pub const DEFAULT_PROJECT_DIR: &str = "project";
pub const SCRIPTS_DIR: &str = "assets/scripts";
pub const TEXTURES_DIR: &str = "assets/textures";
pub const SHADERS_DIR: &str = "assets/shaders";
pub const GENERATED_DIR: &str = "assets/generated";

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Edit,
    /// The player, scenario runs and captures: check the engine, write nothing. A
    /// shipped game never rewrites its project, and a run over the tracked fixture
    /// project leaves the checkout clean.
    Run,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Opened {
    pub root: PathBuf,
    pub engine: EngineCheck,
    pub settings: ProjectSettings,
}

/// Moves the working directory: open before reading any project path, after resolving
/// other relative command-line paths. Progress goes to stderr: `session-mcp`'s stdout is JSON-RPC.
pub fn open(dir: &Path, access: Access) -> Result<Opened, String> {
    let (opened, mut lines) = prepare(dir, access)?;
    let root = &opened.root;
    // lint: allow(statement-opt-out) the working-directory move, not a clock read
    #[expect(
        clippy::disallowed_methods,
        reason = "opening a project makes it the working directory, once, at startup"
    )]
    std::env::set_current_dir(root).map_err(|e| format!("{}: {e}", root.display()))?;
    MOVES.fetch_add(1, Ordering::Relaxed);
    lines.push(format!("Opened {}", root.display()));
    lines.extend(opened.engine.warning());
    for line in lines {
        eprintln!("[Project] {line}");
    }
    if let Some(upgrade) = opened.engine.upgrade().filter(|_| access == Access::Edit) {
        // Logged too, so the console and the MCP replies carry it.
        log::warn!("[Project] {upgrade}");
    }
    Ok(opened)
}

pub fn return_to(dir: &Path) -> std::io::Result<()> {
    // lint: allow(statement-opt-out) the working-directory restore, not a clock read
    #[expect(
        clippy::disallowed_methods,
        reason = "a failed switch restores the working directory `open` moved"
    )]
    std::env::set_current_dir(dir)?;
    MOVES.fetch_add(1, Ordering::Relaxed);
    Ok(())
}

pub fn moves() -> u64 {
    MOVES.load(Ordering::Relaxed)
}

static MOVES: AtomicU64 = AtomicU64::new(0);

pub(crate) fn prepare(dir: &Path, access: Access) -> Result<(Opened, Vec<String>), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let root = std::fs::canonicalize(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    create_skeleton(&root)?;
    let (engine, file, lines) = match access {
        Access::Edit => {
            let (engine, file, mut lines) = file::record(&root)?;
            // lint: allow(gpu-default-only) the agent guide a project is given; nothing is drawn from it
            lines.extend(agent::write(&root, cfg!(feature = "dev")));
            (engine, file, lines)
        }
        Access::Run => {
            let (engine, file) = file::inspect(&root)?;
            (engine, file, Vec::new())
        }
    };
    Ok((
        Opened {
            root,
            engine,
            settings: file.into(),
        },
        lines,
    ))
}

pub fn inspect(dir: &Path) -> Result<EngineCheck, String> {
    Ok(file::inspect(dir)?.0)
}

pub fn is_project(dir: &Path) -> bool {
    dir.join(PROJECT_FILE).is_file() || dir.join(ASSETS_DIR).is_dir()
}

/// The open project's root: the working directory, which [`open`] made the root.
/// (The in-process tests never open one; for them it is wherever they run.)
pub fn root() -> PathBuf {
    std::env::current_dir().unwrap_or_default()
}

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
