//! src/dev/export/ — ship a project: its runtime files, without its convertible
//! sources (#1267).
//!
//! Unity's Build, for rusty's one runtime: the standalone `player` runs a project
//! folder, so a shipped game is a project folder holding only what the player
//! reads. [`export`] brings every converted source (`.fbx`, `.blend`) up to date
//! in the open project's import cache, then copies the project's runtime files
//! into a fresh folder and leaves those sources behind. The exported project has
//! no source a converter reads, so its player converts nothing and never starts
//! Blender, with no mode switch: the player's refresh before Play
//! (`asset::convert::refresh_sources`) finds nothing, and every load is served
//! from the cache (`asset::convert::cached_gltf` and `texture_ref` both resolve a
//! source's cache by its path, the source present or not).
//!
//! What is copied: `project.rusty` (the build settings, and the mark that makes
//! the import cache project-relative), the seed manifest, and everything under
//! `assets/` but the convertible sources, import cache included. What is not:
//! `cache/` (machine-local, regenerable) and `saved/` (this machine's save data).
//! The engine's own content (`engine/`) ships beside the player, not in the
//! project (`core::project::engine_dir`).
//!
//! Authoring, not sim: dev builds only, since it may run Blender. Files are
//! visited and written in sorted order, so two exports of one project are
//! byte-identical.

mod copy;

use std::path::{Path, PathBuf};

use crate::asset::convert::{cached_gltf, convertible_sources};
use crate::core::project::{ASSETS_DIR, PROJECT_FILE};
use crate::scene::io::SEED_MANIFEST_PATH;

/// What an export did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Exported {
    /// Files written into the output, as `/`-separated paths relative to it, in the
    /// order written.
    pub files: Vec<String>,
    /// Convertible sources brought up to date and left behind, project-relative.
    pub sources: Vec<PathBuf>,
}

/// Export the open project (the working directory, `core::project::open`) into
/// `out`, which must be absent or empty and outside the project. A source that
/// fails to convert fails the export, naming it, before anything is written.
pub fn export(out: &Path) -> Result<Exported, String> {
    let root = crate::core::project::root();
    let out = std::path::absolute(out).map_err(|e| format!("{}: {e}", out.display()))?;
    check_out(&root, &out)?;
    if !Path::new(PROJECT_FILE).is_file() {
        return Err(format!(
            "{}: no {PROJECT_FILE}; open the project in the editor once",
            root.display()
        ));
    }
    let sources = convertible_sources(Path::new(ASSETS_DIR));
    for source in &sources {
        cached_gltf(source).map_err(|e| format!("{}: {e}", source.display()))?;
    }
    let files = copy_runtime_files(Path::new("."), &out)?;
    Ok(Exported { files, sources })
}

/// Copy the runtime files of the project at `root` into `out` (see the module
/// doc), and return them in the order written.
fn copy_runtime_files(root: &Path, out: &Path) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    for file in [PROJECT_FILE, SEED_MANIFEST_PATH] {
        if root.join(file).is_file() {
            copy::file(root, Path::new(file), out, &mut files)?;
        }
    }
    copy::tree(root, Path::new(ASSETS_DIR), out, &mut files)?;
    Ok(files)
}

/// Refuse an output that holds anything, or that overlaps the project: a copy
/// into the tree it walks would copy itself.
fn check_out(root: &Path, out: &Path) -> Result<(), String> {
    let canonical = |p: &Path| crate::core::paths::canonicalize(p).unwrap_or(p.to_path_buf());
    let (root, resolved) = (canonical(root), canonical(out));
    if resolved.starts_with(&root) || root.starts_with(&resolved) {
        return Err(format!(
            "{}: the output must lie outside the project {}",
            out.display(),
            root.display()
        ));
    }
    let occupied = std::fs::read_dir(out).is_ok_and(|mut entries| entries.next().is_some());
    if occupied {
        return Err(format!(
            "{}: not empty; export into a new or empty folder",
            out.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
