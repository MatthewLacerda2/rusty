//! The Blender version asked, kept across processes in the project's
//! machine-local [`project::CACHE_DIR`] (Unity's `Library/`), so a launch of a
//! project with `.blend` sources does not start Blender just to read its version.
//!
//! One small file, [`FILE`], holds one answer: the version a Blender reported
//! (accepted or refused below `MIN_MAJOR`), keyed by its executable's canonical
//! path, length and modification time, so an upgrade, or a link that moves to
//! another install, asks again. A launcher that never changes when Blender does
//! (a snap's `/snap/bin/blender` resolves to `snap` itself) would keep a stale
//! answer for good, so an answer is read at most [`READS`] times, then asked
//! again: a count, not an age, since this crate reads no clock (the sim's
//! determinism gate). No answer at all, and an output naming no version, are
//! never kept here. A missing, unreadable or foreign file is a miss, and the next
//! answer is written over it, atomically: two processes writing at once leave
//! one whole file, whichever rename lands last.

use rusty_core::project;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The file, in the project's [`project::CACHE_DIR`].
pub const FILE: &str = "blender-version.json";

/// How many processes may read one answer before Blender is asked again.
pub(super) const READS: u32 = 32;

/// Where the project at `root` keeps the answer, when `root` holds a project.
pub fn in_project(root: &Path) -> Option<PathBuf> {
    root.join(project::PROJECT_FILE)
        .is_file()
        .then(|| root.join(project::CACHE_DIR).join(FILE))
}

/// An answer kept: which executable, as it was, said which version, read how often.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct Known {
    pub path: PathBuf,
    pub len: u64,
    pub modified: Option<Duration>,
    pub version: String,
    pub reads: u32,
}

impl Known {
    /// Whether this answers for the executable at `path` as it is now.
    pub fn answers(&self, path: &Path, len: u64, modified: Option<Duration>) -> bool {
        (self.path.as_path(), self.len, self.modified) == (path, len, modified)
    }
}

/// The answer in `file`, `None` when there is none or it does not parse.
pub(super) fn read(file: &Path) -> Option<Known> {
    serde_json::from_slice(&std::fs::read(file).ok()?).ok()
}

/// Keep `known` in `file`, making its folder.
pub(super) fn write(file: &Path, known: &Known) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let bytes = serde_json::to_vec_pretty(known).map_err(std::io::Error::other)?;
    rusty_core::fs::write_atomic(file, &bytes)
}
