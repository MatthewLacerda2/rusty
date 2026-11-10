//! What this world last read or wrote at each bake sidecar's path (#1982): the
//! one rule every bake file a scene save writes follows (`<scene>.occlusion`,
//! `<scene>.navmesh`, `<scene>.lighting.json`).
//!
//! Unity's lighting data is an asset a scene save never rewrites. rusty keeps a
//! bake beside its scene, so a save writes it only when this world holds bytes
//! other than the ones it last read or wrote at that path: it baked since, the bake
//! was cleared, or it is a Save As to a new path. A save that would only write back
//! what this world loaded leaves the file alone, and when the file changed on disk
//! since (another process baked the scene), it says so once: the newer file stays,
//! and the next load uses it. A world that did bake since still writes its own bake
//! (the last write wins, and the warning says so). Removal follows the same rule: a
//! cleared bake removes the file only when this world read or wrote it and nothing
//! changed it since.
//!
//! "Changed on disk" is the file's length, then its content hash (`core::hash`),
//! never a clock (the sim reads none): a same-length rewrite is caught too.
//!
//! Keyed by path, so a Save As keeps the old path's record, and a Play load adds
//! its own without touching the edit scene's (Stop puts the record back with the
//! scene). An edit-mode load starts afresh. Interior mutability because a save
//! takes `&Scene`, as [`DiskText`](super::DiskText) does; never serialized.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::core::hash::content;

/// A file's bytes, as compared: their length and content hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stamp {
    len: u64,
    hash: u64,
}

impl Stamp {
    fn of(bytes: &[u8]) -> Self {
        Self {
            len: bytes.len() as u64,
            hash: content(bytes),
        }
    }
}

/// What this world saw at one path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Seen {
    /// The file as it was (`None`: no file).
    disk: Option<Stamp>,
    /// What this world would write right after (`None`: no bake), so a save tells
    /// a bake since from a format that does not round-trip byte for byte.
    held: Option<u64>,
    /// This world baked since, so its bake is written even when its bytes match
    /// what it held (a re-bake of an unchanged scene over another's newer file).
    baked: bool,
}

/// What [`BakeFiles::store`] did to the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Left as it was: it holds what this world holds, a newer bake, or a file
    /// this world never saw.
    Kept,
    /// Written with this world's bytes.
    Written,
    /// Removed: this world's bake was cleared.
    Removed,
}

/// What a save did, and the warning it logged when the file had changed on disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    pub action: Action,
    pub warning: Option<String>,
}

/// The records, one per sidecar path this world read or wrote.
#[derive(Clone, Debug, Default)]
pub struct BakeFiles {
    seen: RefCell<BTreeMap<PathBuf, Seen>>,
    /// Paths whose newer file a save already warned about, until the next record.
    warned: RefCell<BTreeSet<PathBuf>>,
}

impl BakeFiles {
    /// Record that `path` held `disk` (`None`: no file) when this world read it,
    /// and that this world then held `held` (what its next save would write).
    pub fn loaded(&self, path: &Path, disk: Option<&[u8]>, held: Option<&[u8]>) {
        let seen = Seen {
            disk: disk.map(Stamp::of),
            held: held.map(content),
            baked: false,
        };
        self.seen.borrow_mut().insert(path.to_path_buf(), seen);
        self.warned.borrow_mut().remove(path);
    }

    /// This world just baked what it keeps at `path`: its next save writes it,
    /// whatever it held before (the last write wins).
    pub fn baked(&self, path: &Path) {
        if let Some(seen) = self.seen.borrow_mut().get_mut(path) {
            seen.baked = true;
        }
    }

    /// Forget `path`: its file could not be read, so a save never removes it.
    pub fn forget(&self, path: &Path) {
        self.seen.borrow_mut().remove(path);
        self.warned.borrow_mut().remove(path);
    }

    /// Bring `path` to `want` (`None`: no bake) by the rule above. `tag` names
    /// the bake in the warning (`"Occlusion"`).
    pub fn store(&self, path: &Path, want: Option<&[u8]>, tag: &str) -> Result<Saved, String> {
        let seen = self.seen.borrow().get(path).copied();
        let newer = seen.is_some_and(|s| !disk_holds(path, s.disk));
        let kept = |warning| Saved {
            action: Action::Kept,
            warning,
        };
        let unchanged = seen.is_some_and(|s| !s.baked && s.held == want.map(content));
        // Nothing to write: as seen, or no bake for a file this world never saw.
        if unchanged || (want.is_none() && seen.is_none()) {
            return Ok(kept(newer.then(|| self.warn_newer(path, tag)).flatten()));
        }
        let Some(bytes) = want else {
            if newer {
                return Ok(kept(self.warn_newer(path, tag)));
            }
            if path.exists() {
                std::fs::remove_file(path)
                    .map_err(|e| format!("Failed to remove {}: {e}", path.display()))?;
            }
            self.loaded(path, None, None);
            return Ok(Saved {
                action: Action::Removed,
                warning: None,
            });
        };
        // lint: allow(asset-write) a bake sidecar, a document read whole on load
        std::fs::write(path, bytes)
            .map_err(|e| format!("Failed to write {}: {e}", path.display()))?;
        self.loaded(path, Some(bytes), Some(bytes));
        let warning = newer.then(|| {
            let text = format!(
                "[{tag}] {} had changed on disk since this scene read it (another \
                 process baked it?): this scene's own bake overwrote it, the last write wins",
                path.display()
            );
            // lint: allow(log-tag) `text` opens with the caller's tag, `[Occlusion]` and the like
            log::warn!("{text}");
            text
        });
        Ok(Saved {
            action: Action::Written,
            warning,
        })
    }

    /// Warn, once per record, that the file at `path` is newer than this world's.
    fn warn_newer(&self, path: &Path, tag: &str) -> Option<String> {
        if !self.warned.borrow_mut().insert(path.to_path_buf()) {
            return None;
        }
        let text = format!(
            "[{tag}] {} changed on disk since this scene read it (another process \
             baked it?): the save keeps that newer bake, and the next load uses it",
            path.display()
        );
        // lint: allow(log-tag) `text` opens with the caller's tag, `[Occlusion]` and the like
        log::warn!("{text}");
        Some(text)
    }

    /// Whether a save warned that the file at `path` is newer than this world's.
    #[cfg(test)]
    pub fn warned(&self, path: &Path) -> bool {
        self.warned.borrow().contains(path)
    }

    /// Whether this world holds a record for `path`.
    #[cfg(test)]
    pub fn has(&self, path: &Path) -> bool {
        self.seen.borrow().contains_key(path)
    }
}

/// Whether the file at `path` is `stamp` (`None`: no file). A file that cannot be
/// read is not: a save then never writes over it with bytes it only loaded.
fn disk_holds(path: &Path, stamp: Option<Stamp>) -> bool {
    match (std::fs::metadata(path), stamp) {
        (Err(e), None) => e.kind() == std::io::ErrorKind::NotFound,
        (Ok(m), Some(s)) if m.is_file() && m.len() == s.len => {
            std::fs::read(path).is_ok_and(|bytes| content(&bytes) == s.hash)
        }
        _ => false,
    }
}

#[cfg(test)]
#[path = "bake_files_tests.rs"]
mod tests;
