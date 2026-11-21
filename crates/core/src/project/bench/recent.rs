//! stored-format: the recent-projects list in the user's config directory
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const RECENT_FILE: &str = "recent_projects.json";
pub const MAX_RECENT: usize = 32;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentProject {
    pub path: PathBuf,
    /// When it was last opened, in Unix seconds.
    pub opened_at: u64,
}

impl RecentProject {
    pub fn name(&self) -> String {
        self.path.file_name().map_or_else(
            || self.path.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        )
    }

    pub fn is_missing(&self) -> bool {
        !self.path.is_dir()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentProjects {
    pub projects: Vec<RecentProject>,
}

impl RecentProjects {
    /// Read the list at `path`. A missing or unreadable file is an empty list: the
    /// list is a convenience, and a broken one must never keep the editor from starting.
    pub fn load(path: &Path) -> Self {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        serde_json::from_str(&text).unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let text = serde_json::to_string_pretty(self).unwrap_or_default();
        // lint: allow(asset-write) project settings, not read by a path-keyed cache
        crate::fs::write_atomic(path, (text + "\n").as_bytes())
            .map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn touch(&mut self, root: &Path, now: u64) {
        self.remove(root);
        let entry = RecentProject {
            path: root.to_path_buf(),
            opened_at: now,
        };
        self.projects.insert(0, entry);
        self.projects.truncate(MAX_RECENT);
    }

    pub fn remove(&mut self, root: &Path) {
        self.projects.retain(|p| p.path != root);
    }
}

pub fn config_dir(var: impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let nonempty = |name: &str| var(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    let base = if cfg!(target_os = "macos") {
        nonempty("HOME").map(|home| home.join("Library").join("Application Support"))
    } else {
        nonempty("XDG_CONFIG_HOME").or_else(|| nonempty("HOME").map(|h| h.join(".config")))
    };
    base.map(|dir| dir.join("rusty"))
}

pub fn default_path() -> Option<PathBuf> {
    config_dir(|name| std::env::var(name).ok()).map(|dir| dir.join(RECENT_FILE))
}

/// Record that `root` was opened at `now` in this user's list. Errors are the
/// caller's to log: a list that can't be written never stops a project opening.
pub fn record(root: &Path, now: u64) -> Result<(), String> {
    let path = default_path().ok_or("no home directory to keep the recent projects in")?;
    let mut list = RecentProjects::load(&path);
    list.touch(root, now);
    // lint: allow(asset-write) project settings, not read by a path-keyed cache
    list.save(&path)
}

/// How long ago `then` was, as Unity Hub's list says it: "just now", "5 minutes
/// ago", "3 days ago".
pub fn ago(then: u64, now: u64) -> String {
    let secs = now.saturating_sub(then);
    let units = [
        (365 * 86_400, "year"),
        (30 * 86_400, "month"),
        (7 * 86_400, "week"),
        (86_400, "day"),
        (3_600, "hour"),
        (60, "minute"),
    ];
    let Some((count, unit)) = units
        .iter()
        .map(|&(len, unit)| (secs / len, unit))
        .find(|&(count, _)| count > 0)
    else {
        return "just now".to_string();
    };
    let plural = if count == 1 { "" } else { "s" };
    format!("{count} {unit}{plural} ago")
}

#[cfg(test)]
#[path = "recent_tests.rs"]
mod tests;
