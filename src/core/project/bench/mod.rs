//! src/core/project/bench/ — **rusty bench**: the projects a user works on (#942).
//!
//! Unity Hub's Projects page without the window: the projects opened before
//! ([`recent`]), making a new one, opening an existing one and forgetting an entry.
//! The editor's project picker (`editor::project_picker`, #854) draws it and the
//! session MCP's `bench_*` tools (`dev::mcp::bench`) offer it to an agent; both call
//! this code, so the two can never disagree about what *New Project* makes or which
//! folder opens. (Not `make bench`, which measures frame time: `dev::bench`.)
//!
//! Times are Unix seconds handed in by the caller: `core` is sim and never reads a
//! clock.

pub mod recent;

use std::path::{Path, PathBuf};

use super::Access;
pub use recent::{RecentProject, RecentProjects};

/// The folder *New Project* makes: `name` under `parent`. Refused when the name is
/// empty, hidden or a path, or when the folder already holds something (Unity Hub
/// refuses a non-empty folder too).
pub fn new_project_dir(parent: &Path, name: &str) -> Result<PathBuf, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Name the project".to_string());
    }
    if name.starts_with('.') || name.contains(['/', '\\']) {
        return Err(format!("{name:?} is not a folder name"));
    }
    let dir = parent.join(name);
    let occupied = std::fs::read_dir(&dir).is_ok_and(|mut entries| entries.next().is_some());
    if occupied || dir.is_file() {
        return Err(format!("{} already exists and is not empty", dir.display()));
    }
    Ok(dir)
}

/// Whether an existing folder may be opened: `Err` when it is not a project, else
/// the engine warning to confirm first (`None` unless a newer engine or a branch
/// build last opened it: an upgrade needs no confirming, #1319).
pub fn check_existing(dir: &Path) -> Result<Option<String>, String> {
    if !dir.is_dir() {
        return Err(format!("{} does not exist", dir.display()));
    }
    if !super::is_project(dir) {
        let file = super::PROJECT_FILE;
        return Err(format!(
            "{} is not a rusty project (no {file} or assets/): use New Project to make one",
            dir.display()
        ));
    }
    Ok(super::inspect(dir)?.warning())
}

/// A user's bench: their recent list and where it is saved.
pub struct Bench {
    pub recent: RecentProjects,
    /// Where changes are saved; `None` keeps them in memory.
    path: Option<PathBuf>,
}

impl Bench {
    /// The bench whose list lives at `path` (`None`: an empty, in-memory one).
    pub fn at(path: Option<PathBuf>) -> Self {
        let recent = path
            .as_deref()
            .map(RecentProjects::load)
            .unwrap_or_default();
        Self { recent, path }
    }

    /// This user's bench, from [`recent::default_path`].
    pub fn for_user() -> Self {
        Self::at(recent::default_path())
    }

    /// *New Project*: make `name` under `parent` a project (its skeleton,
    /// `project.rusty`, `CLAUDE.md` and in a `dev` build `.mcp.json`, as opening it
    /// would) without opening it, and put it at the
    /// top of the list. Returns its root.
    pub fn create(&mut self, parent: &Path, name: &str, now: u64) -> Result<PathBuf, String> {
        let dir = new_project_dir(parent, name)?;
        let (opened, _) = super::prepare(&dir, Access::Edit)?;
        self.recent.touch(&opened.root, now);
        self.save()?;
        Ok(opened.root)
    }

    /// *Open*: check `dir` is a project ([`check_existing`]) and put it at the top of
    /// the list. Returns its root and the engine warning, if a newer engine or a
    /// branch build last opened it. Nothing in the folder is written.
    pub fn open(&mut self, dir: &Path, now: u64) -> Result<(PathBuf, Option<String>), String> {
        let warning = check_existing(dir)?;
        let root = canonical(dir);
        self.recent.touch(&root, now);
        self.save()?;
        Ok((root, warning))
    }

    /// Take `dir` off the list (the folder is left alone). `false` when it was not
    /// on it.
    pub fn forget(&mut self, dir: &Path) -> Result<bool, String> {
        let before = self.recent.projects.len();
        self.recent.remove(dir);
        self.recent.remove(&canonical(dir));
        let removed = self.recent.projects.len() < before;
        if removed {
            self.save()?;
        }
        Ok(removed)
    }

    fn save(&self) -> Result<(), String> {
        match &self.path {
            // lint: allow(asset-write) project settings, not read by a path-keyed cache
            Some(path) => self.recent.save(path),
            None => Ok(()),
        }
    }
}

/// `dir` as the list stores it: absolute and resolved, or as given once it is gone.
fn canonical(dir: &Path) -> PathBuf {
    crate::core::paths::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf())
}

#[cfg(test)]
mod tests;
