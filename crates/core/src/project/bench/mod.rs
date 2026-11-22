//! Unity Hub's projects list, for the editor's picker and the MCP's `bench_*` tools
//! alike; not `make bench`, which is `dev::bench`.
pub mod recent;

use std::path::{Path, PathBuf};

use super::Access;
pub use recent::{RecentProject, RecentProjects};

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

pub struct Bench {
    pub recent: RecentProjects,
    path: Option<PathBuf>,
}

impl Bench {
    pub fn at(path: Option<PathBuf>) -> Self {
        let recent = path
            .as_deref()
            .map(RecentProjects::load)
            .unwrap_or_default();
        Self { recent, path }
    }

    pub fn for_user() -> Self {
        Self::at(recent::default_path())
    }

    pub fn create(&mut self, parent: &Path, name: &str, now: u64) -> Result<PathBuf, String> {
        let dir = new_project_dir(parent, name)?;
        let (opened, _) = super::prepare(&dir, Access::Edit)?;
        self.recent.touch(&opened.root, now);
        self.save()?;
        Ok(opened.root)
    }

    pub fn open(&mut self, dir: &Path, now: u64) -> Result<(PathBuf, Option<String>), String> {
        let warning = check_existing(dir)?;
        let root = canonical(dir);
        self.recent.touch(&root, now);
        self.save()?;
        Ok((root, warning))
    }

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

fn canonical(dir: &Path) -> PathBuf {
    std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf())
}

#[cfg(test)]
mod tests;
