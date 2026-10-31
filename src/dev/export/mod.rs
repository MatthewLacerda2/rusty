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
//! source's cache by its path, the source present or not). Every image's
//! compressed copy is made current first and shipped beside it (#1574), so the
//! player never encodes a texture nor writes one into its install. Every baked
//! shader stale against the engine's shaders is re-baked from its recipe the same
//! way (#1638, `shadergen::rebake::refresh_all`), so the player composes none; one
//! that fails is warned of and shipped stale, and the player re-composes it.
//!
//! What is copied: `project.rusty` (the build settings, and the mark that makes
//! the import cache project-relative), the seed manifest, everything under
//! `assets/` but the convertible sources (and Blender's backups of them), and the
//! import cache of each source and each image, under its lock; a cache whose
//! source is gone stays behind. What is not: `cache/` (machine-local, regenerable) and `saved/` (this
//! machine's save data), which the export makes empty, with the rest of the
//! project skeleton, so the player's first launch writes nothing into an install.
//! The engine's own content (`engine/`) ships beside the player, not in the
//! project (`core::project::engine_dir`).
//!
//! Authoring, not sim: dev builds only, since it may run Blender. Files are
//! visited and written in sorted order, so two exports of one project are
//! byte-identical.

mod copy;
mod textures;

use std::path::{Path, PathBuf};

use crate::asset::convert::{cached_gltf, convertible_sources};
use crate::core::paths;
use crate::core::project::{create_skeleton, is_project, ASSETS_DIR, PROJECT_FILE, SHADERS_DIR};
use crate::scene::io::SEED_MANIFEST_PATH;

/// What an export did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Exported {
    /// Files written into the output, as `/`-separated paths relative to it, in the
    /// order written (the project skeleton's folders aside).
    pub files: Vec<String>,
    /// Convertible sources brought up to date and left behind, project-relative.
    pub sources: Vec<PathBuf>,
}

/// Export the open project (the working directory, `core::project::open`) into
/// `out`, which must pass [`check`]. A source that fails to convert fails the
/// export, naming it, before anything is written; a texture a cache names outside
/// what was copied fails it after, naming each, and the output is emptied again.
pub fn export(out: &Path) -> Result<Exported, String> {
    let out = std::path::absolute(out).map_err(|e| format!("{}: {e}", out.display()))?;
    check(&crate::core::project::root(), &out)?;
    let sources = convertible_sources(Path::new(ASSETS_DIR));
    for source in &sources {
        cached_gltf(source).map_err(|e| format!("{}: {e}", source.display()))?;
    }
    // An image that fails is warned of and ships uncompressed, as Unity builds
    // with an import error logged.
    crate::asset::texture_import::refresh_all(Path::new(ASSETS_DIR));
    // A baked shader stale against this engine's shaders is re-baked from its
    // recipe, so the player loads it as shipped (#1638); one that fails is
    // warned of and ships as it is.
    crate::shadergen::rebake::refresh_all(&paths::native(SHADERS_DIR));
    let existed = out.is_dir();
    let written = copy_runtime_files(Path::new("."), &sources, &out).and_then(|files| {
        create_skeleton(&out)?;
        Ok(files)
    });
    match written {
        Ok(files) => Ok(Exported { files, sources }),
        Err(e) => {
            // `out` was absent or empty (`check`): leave it so.
            let _ = std::fs::remove_dir_all(&out);
            if existed {
                let _ = std::fs::create_dir(&out);
            }
            Err(e)
        }
    }
}

/// Refuse a `project` that is not one (`core::project::is_project`), and an `out`
/// that is a file, holds anything or overlaps the project: what the `export`
/// binary asks before it opens the project, so a refusal writes nothing into it.
pub fn check(project: &Path, out: &Path) -> Result<(), String> {
    if !is_project(project) {
        return Err(format!(
            "{}: not a rusty project (no {PROJECT_FILE} and no {ASSETS_DIR}/)",
            project.display()
        ));
    }
    check_out(project, out)
}

/// Copy the runtime files of the project at `root` into `out` (see the module
/// doc), the import caches of `sources` (project-relative) among them, and return
/// them in the order written. Fails naming every texture a copied cache names
/// that was not copied.
fn copy_runtime_files(root: &Path, sources: &[PathBuf], out: &Path) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    for file in [PROJECT_FILE, SEED_MANIFEST_PATH] {
        if root.join(file).is_file() {
            copy::file(root, Path::new(file), out, &mut files)?;
        }
    }
    copy::tree(root, Path::new(ASSETS_DIR), out, &mut files)?;
    let glbs = copy::caches(root, sources, out, &mut files)?;
    let images = crate::asset::texture_import::images(&root.join(ASSETS_DIR));
    let images: Vec<PathBuf> = images
        .iter()
        .map(|i| i.strip_prefix(root).unwrap_or(i).to_path_buf())
        .collect();
    copy::texture_caches(root, &images, out, &mut files)?;
    let missing = textures::missing(out, &glbs, &files);
    if !missing.is_empty() {
        return Err(format!(
            "textures outside the exported folders (move them under {ASSETS_DIR}/):\n  {}",
            missing.join("\n  ")
        ));
    }
    Ok(files)
}

/// Refuse an output that is a file, holds anything, or overlaps the project: a
/// copy into the tree it walks would copy itself. Compared through symlinks, the
/// output by its nearest folder that exists.
fn check_out(root: &Path, out: &Path) -> Result<(), String> {
    if out.exists() && !out.is_dir() {
        return Err(format!(
            "{}: a file; export into a new or empty folder",
            out.display()
        ));
    }
    let (root, resolved) = (resolved(root), resolved(out));
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

/// `path` with its nearest existing ancestor canonicalized (symlinks resolved)
/// and the rest, which does not exist yet, folded on.
fn resolved(path: &Path) -> PathBuf {
    let path = paths::fold(&std::path::absolute(path).unwrap_or(path.to_path_buf()));
    for ancestor in path.ancestors() {
        if let Ok(canonical) = paths::canonicalize(ancestor) {
            let rest = path.strip_prefix(ancestor).unwrap_or(Path::new(""));
            return canonical.join(rest);
        }
    }
    path
}

#[cfg(test)]
mod tests;
