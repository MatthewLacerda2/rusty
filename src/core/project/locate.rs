//! src/core/project/locate.rs — where the project and the engine's content are.
//!
//! Two lookups that must work from any working directory, in a checkout and in a
//! shipped build alike (macOS and Linux):
//!
//! - **The engine's content** (`engine/`: shaders, bundled scripts, the preview
//!   model) sits next to the executable or above it. A shipped build carries it as
//!   `<exe dir>/engine` (macOS: `Contents/Resources/engine`); a dev build finds the
//!   checkout's `engine/` by walking up from `target/<profile>/`.
//! - **The project** a binary opens when none is named: a packaged `project/` next to
//!   the executable (the shipped game), else `./project`.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use super::{ASSETS_DIR, DEFAULT_PROJECT_DIR};

/// The engine content folder's name, in a checkout and in a shipped build.
const ENGINE_DIR: &str = "engine";
/// A file every engine content folder holds; finding it proves the folder is one.
const ENGINE_MARKER: &str = "shaders/common.wgsl";

/// The engine's content folder, absolute. Looked up once per process: next to the
/// executable or any folder above it, then the macOS bundle's `Resources/`, then the
/// checkout this binary was built from.
pub fn engine_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let built_from = Path::new(env!("CARGO_MANIFEST_DIR")).join(ENGINE_DIR);
        exe_dir()
            .and_then(|exe| find_engine_dir(&exe))
            .unwrap_or(built_from)
    })
}

/// `rel` under [`engine_dir`], as a `/`-separated string for the APIs that take one.
pub fn engine_path(rel: &str) -> String {
    format!("{}/{rel}", engine_dir().to_string_lossy())
}

/// The project to open: `named` when the command line gave one, else a packaged
/// project next to the executable, else [`DEFAULT_PROJECT_DIR`].
pub fn locate(named: Option<PathBuf>) -> PathBuf {
    named
        .or_else(packaged_project)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_PROJECT_DIR))
}

/// A shipped game's project: `project/` beside the executable, or in the macOS
/// bundle's `Resources/`, when it holds an `assets/` folder.
pub fn packaged_project() -> Option<PathBuf> {
    find_packaged_project(&exe_dir()?)
}

fn exe_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
    exe.parent().map(Path::to_path_buf)
}

/// The first `engine/` holding the marker in `exe_dir` or above it, then in the
/// macOS bundle's `Resources/`.
pub(super) fn find_engine_dir(exe_dir: &Path) -> Option<PathBuf> {
    let bundle = exe_dir.join("../Resources").join(ENGINE_DIR);
    exe_dir
        .ancestors()
        .map(|dir| dir.join(ENGINE_DIR))
        .chain(std::iter::once(bundle))
        .find(|dir| dir.join(ENGINE_MARKER).is_file())
}

pub(super) fn find_packaged_project(exe_dir: &Path) -> Option<PathBuf> {
    [exe_dir.to_path_buf(), exe_dir.join("../Resources")]
        .into_iter()
        .map(|dir| dir.join(DEFAULT_PROJECT_DIR))
        .find(|dir| dir.join(ASSETS_DIR).is_dir())
}
