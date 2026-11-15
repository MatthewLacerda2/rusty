//! crates/core/src/project/content.rs — the `engine/…` spelling a stored path takes to
//! name the engine's own content (#2185).
//!
//! A path a project stores (a texture recipe's graph or image, a bloom dirt) is
//! read from the project root, except one that starts with [`ENGINE_PREFIX`]: that
//! one is read from the engine's content folder ([`super::engine_dir`]), which ships
//! beside every install. The stored path keeps the `engine/…` spelling, so it names
//! the same file on every machine. The engine wins: a project folder named
//! `engine/` is unreachable through such a path, since a fallback would make one
//! path mean two files.

use std::path::{Path, PathBuf};

/// What a stored path starts with to name a file under the engine's content folder.
pub const ENGINE_PREFIX: &str = "engine/";

/// `path` without [`ENGINE_PREFIX`], when it names engine content.
pub fn engine_relative(path: &str) -> Option<&str> {
    path.strip_prefix(ENGINE_PREFIX)
}

/// The two folders a stored path is read from: the project root, and the engine's
/// content folder for an `engine/…` path. Tests hand it a temp engine folder;
/// everything else takes [`ContentRoots::new`], the running engine's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentRoots {
    pub project: PathBuf,
    pub engine: PathBuf,
}

impl ContentRoots {
    /// `project` with the running engine's content folder ([`super::engine_dir`]).
    pub fn new(project: &Path) -> Self {
        Self::with_engine(project, super::engine_dir())
    }

    pub fn with_engine(project: &Path, engine: &Path) -> Self {
        Self {
            project: project.to_path_buf(),
            engine: engine.to_path_buf(),
        }
    }

    /// The folder `path` is named from: the engine's for an `engine/…` path, else
    /// the project's. A file outside it is outside what the seed covers.
    pub fn base(&self, path: &str) -> &Path {
        if engine_relative(path).is_some() {
            &self.engine
        } else {
            &self.project
        }
    }

    /// The file `path` names.
    pub fn file(&self, path: &str) -> PathBuf {
        let relative = engine_relative(path).unwrap_or(path);
        self.base(path).join(relative)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_engine_path_reads_from_the_engine_and_any_other_from_the_project() {
        let roots = ContentRoots::with_engine(Path::new("/game"), Path::new("/install/engine"));
        let engine = roots.file("engine/textures/library/rust.graph.json");
        assert_eq!(
            engine,
            Path::new("/install/engine/textures/library/rust.graph.json")
        );
        assert_eq!(roots.base("engine/a.png"), Path::new("/install/engine"));
        assert_eq!(roots.file("assets/a.png"), Path::new("/game/assets/a.png"));
        // Only the prefix counts: a folder merely named like it is the project's.
        assert_eq!(
            roots.file("engines/a.png"),
            Path::new("/game/engines/a.png")
        );
        assert_eq!(
            roots.file("assets/engine/a.png"),
            Path::new("/game/assets/engine/a.png")
        );
    }

    #[test]
    fn an_empty_project_root_leaves_a_project_path_as_written() {
        let roots = ContentRoots::with_engine(Path::new(""), Path::new("/e"));
        assert_eq!(roots.file("assets/dirt.png"), Path::new("assets/dirt.png"));
        assert_eq!(roots.file("engine/dirt.png"), Path::new("/e/dirt.png"));
    }
}
