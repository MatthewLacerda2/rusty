use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use super::{ASSETS_DIR, DEFAULT_PROJECT_DIR};

const ENGINE_DIR: &str = "engine";
const ENGINE_MARKER: &str = "shaders/common.wgsl";

pub fn engine_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let built_from = checkout().join(ENGINE_DIR);
        exe_dir()
            .and_then(|exe| find_engine_dir(&exe))
            .unwrap_or(built_from)
    })
}

pub fn checkout() -> &'static Path {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest.ancestors().nth(2).unwrap_or(manifest)
}

pub fn engine_path(rel: &str) -> String {
    format!("{}/{rel}", crate::paths::to_slash(engine_dir()))
}

pub fn locate(named: Option<PathBuf>) -> PathBuf {
    named
        .or_else(packaged_project)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_PROJECT_DIR))
}

fn packaged_project() -> Option<PathBuf> {
    find_packaged_project(&exe_dir()?)
}

fn exe_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe = std::fs::canonicalize(&exe).unwrap_or(exe);
    exe.parent().map(Path::to_path_buf)
}

pub(super) fn find_engine_dir(exe_dir: &Path) -> Option<PathBuf> {
    let bundle = exe_dir.join("..").join("Resources").join(ENGINE_DIR);
    exe_dir
        .ancestors()
        .map(|dir| dir.join(ENGINE_DIR))
        .chain(std::iter::once(bundle))
        .find(|dir| dir.join(ENGINE_MARKER).is_file())
}

pub(super) fn find_packaged_project(exe_dir: &Path) -> Option<PathBuf> {
    [exe_dir.to_path_buf(), exe_dir.join("..").join("Resources")]
        .into_iter()
        .map(|dir| dir.join(DEFAULT_PROJECT_DIR))
        .find(|dir| dir.join(ASSETS_DIR).is_dir())
}
