//! A watched folder, and how a path `notify` reports under it is spelled for the
//! caches (#1327).

use std::path::{Path, PathBuf};

/// A watched folder as it was given, and as `notify` reports paths under it: the
/// native backends report absolute paths (canonical on macOS), `PollWatcher` keeps
/// the root as given.
pub(super) struct Root {
    pub(super) given: PathBuf,
    absolute: PathBuf,
    canonical: Option<PathBuf>,
}

impl Root {
    pub(super) fn new(given: &Path) -> Self {
        let absolute = std::env::current_dir()
            .map(|cwd| cwd.join(given))
            .unwrap_or_else(|_| given.to_path_buf());
        Self {
            given: given.to_path_buf(),
            absolute,
            canonical: std::fs::canonicalize(given).ok(),
        }
    }

    /// `path` re-spelled under this root as it was given, if it lies under it.
    pub(super) fn spell(&self, path: &Path) -> Option<PathBuf> {
        if path.starts_with(&self.given) {
            return Some(path.to_path_buf());
        }
        let rest = path.strip_prefix(&self.absolute).ok().or_else(|| {
            let canonical = self.canonical.as_ref()?;
            path.strip_prefix(canonical).ok()
        })?;
        Some(self.given.join(rest))
    }
}

/// Every file under `dir`, recursively, spelled under it; `dir` itself when it is a
/// file (or gone). A symlinked folder is not followed, so a link loop ends.
pub(super) fn files_under(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        found.push(dir.to_path_buf());
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => files_under(&path, found),
            _ => found.push(path),
        }
    }
}
