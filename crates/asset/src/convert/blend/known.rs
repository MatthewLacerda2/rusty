//! Keyed by the executable's path as found, canonicalized, plus its length and mtime. A refused
//! version is never kept, so the upgrade its message asks for is seen at once.
//! An answer is read at most [`READS`] times, then asked again: a launcher that never changes
//! with Blender (snap's `/snap/bin/blender` is `snap` itself) would keep a stale one for good. A
//! count, not an age (this crate reads no clock); a read whose count cannot be written is no read.

use rusty_core::project;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const FILE: &str = "blender-version.json";

pub(super) const READS: u32 = 32;

pub fn in_project(root: &Path) -> Option<PathBuf> {
    root.join(project::PROJECT_FILE)
        .is_file()
        .then(|| root.join(project::CACHE_DIR).join(FILE))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct Known {
    pub path: PathBuf,
    pub canonical: PathBuf,
    pub len: u64,
    pub modified: Option<Duration>,
    pub version: String,
    pub reads: u32,
}

impl Known {
    pub fn answers(
        &self,
        (path, canonical): (&Path, &Path),
        len: u64,
        modified: Option<Duration>,
    ) -> bool {
        let kept = (self.path.as_path(), self.canonical.as_path());
        (kept, self.len, self.modified) == ((path, canonical), len, modified)
    }
}

pub(super) fn read(file: &Path) -> Option<Known> {
    serde_json::from_slice(&std::fs::read(file).ok()?).ok()
}

pub(super) fn write(file: &Path, known: &Known) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let bytes = serde_json::to_vec_pretty(known).map_err(std::io::Error::other)?;
    rusty_core::fs::write_cache(file, &bytes)
}
