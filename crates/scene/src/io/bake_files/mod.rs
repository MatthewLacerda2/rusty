//! What this world last read or wrote at each bake sidecar's path (#1982): the
//! one rule the bake files a scene save writes follow (`<scene>.occlusion`,
//! `<scene>.navmesh`). The SH sidecar, `<scene>.lighting.json`, is not one of them:
//! it is half of the scene document (its entries are index-aligned to the probes the
//! document places), so it is written with the document, always.
//!
//! Unity's lighting data is an asset a scene save never rewrites. rusty keeps a
//! bake beside its scene, so a save writes it only when this world holds bytes
//! other than the ones it last read or wrote at that path, or it baked since
//! ([`BakeFiles::baked`]). A save that would only write back what this world loaded
//! leaves the file alone, and when the file changed on disk since (another process
//! baked the scene), it says so once: the newer file stays, and the next load uses
//! it. A world that did bake since still writes its own bake, and a bake that kept
//! none removes the file (the last write wins, and the warning says so). A file
//! removed outside the engine is written back when this world holds a bake. A file
//! this world refused (corrupt, truncated) is rewritten or removed by the next save
//! while it is still the refused file; a newer one written there since is kept, as
//! after a load. One it could not read at all is kept until this world bakes.
//!
//! The keep-newer rule protects only a file written after this world's own read of
//! that path. A save to a path this world has no record for (a new scene saved over
//! a baked one, Save As, Reset Scene) brings the path to what this world holds, and
//! removes the bake files there it does not hold, as Unity drops a replaced scene's
//! lighting data.
//!
//! "Changed on disk" is the file's length, then its content hash (`core::hash`),
//! never a clock (the sim reads none): a same-length rewrite is caught too. Writes
//! go through `core::fs::write_atomic`, which syncs the file before its rename and
//! its folder after, so a power cut leaves the old file or the new one, never half.
//!
//! Keyed by one spelling of each path (absolute, folded), so a relative and an
//! absolute spelling share a record, a Save As keeps the old path's, and a Play load
//! adds its own without touching the edit scene's (Stop puts the record back with
//! the scene). An edit-mode load starts afresh. Interior mutability because a save
//! takes `&Scene`, as [`DiskText`](super::DiskText) does; never serialized.

mod disk;
mod record;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use disk::{change, key, Change, Disk, Stamp};
use record::{Mark, Seen, Want};
use rusty_core::hash::content;

/// What [`BakeFiles::store`] did to the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Left as it was: it holds what this world holds, a newer bake, or a file
    /// this world could not read.
    Kept,
    /// Written with this world's bytes.
    Written,
    /// Removed: this world holds no bake there.
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
    /// and that this world then held `held` (what its next save would write). The
    /// same slice for both is hashed once.
    pub fn loaded(&self, path: &Path, disk: Option<&[u8]>, held: Option<&[u8]>) {
        let stamp = disk.map(Stamp::of);
        let held = match (disk, held, stamp) {
            (Some(d), Some(h), Some(s)) if std::ptr::eq(d, h) => Some(s.hash),
            _ => held.map(content),
        };
        let disk = stamp.map_or(Disk::Missing, Disk::File);
        self.record(path, disk, held, None, Mark::Loaded);
    }

    /// Record that `path` held `bytes` (`None`: no file), which this world now
    /// holds in that shared buffer: a save of the same buffer hashes nothing.
    pub fn loaded_shared(&self, path: &Path, bytes: Option<&Arc<[u8]>>) {
        let stamp = bytes.map(|b| Stamp::of(b));
        let disk = stamp.map_or(Disk::Missing, Disk::File);
        let held = stamp.map(|s| s.hash);
        self.record(path, disk, held, bytes.cloned(), Mark::Loaded);
    }

    /// Record that the file at `path` was read and refused (`bytes` did not
    /// decode): the next save rewrites it with this world's bake or removes it,
    /// while it is still that file.
    pub fn refused(&self, path: &Path, bytes: &[u8]) {
        self.record(
            path,
            Disk::File(Stamp::of(bytes)),
            None,
            None,
            Mark::Refused,
        );
    }

    /// Record that the file at `path` is there but could not be read: a save keeps
    /// it until this world bakes.
    pub fn unreadable(&self, path: &Path) {
        self.record(path, Disk::Unreadable, None, None, Mark::Loaded);
    }

    /// This world just baked the sidecar ending in `suffix` (`".occlusion"`): every
    /// path it holds a record for gets its bake on the next save, whatever it held
    /// before (the last write wins). A path with no record gets it anyway.
    pub fn baked(&self, suffix: &str) {
        let mut seen = self.seen.borrow_mut();
        let ours = seen
            .iter_mut()
            .filter(|(p, _)| p.as_os_str().to_string_lossy().ends_with(suffix));
        ours.for_each(|(_, s)| {
            s.mark = Mark::Baked;
            // The bake replaced the buffer: holding the old one would only keep it alive.
            s.buffer = None;
        });
    }

    fn record(
        &self,
        path: &Path,
        disk: Disk,
        held: Option<u64>,
        buffer: Option<Arc<[u8]>>,
        mark: Mark,
    ) {
        let seen = Seen {
            disk,
            held,
            buffer,
            mark,
        };
        let path = key(path);
        self.warned.borrow_mut().remove(&path);
        self.seen.borrow_mut().insert(path, seen);
    }

    /// Bring `path` to `want` (`None`: no bake) by the rule above. `tag` names
    /// the bake in the warning (`"Occlusion"`).
    pub fn store(&self, path: &Path, want: Option<&[u8]>, tag: &str) -> Result<Saved, String> {
        self.store_want(path, want.map(Want::Bytes), tag)
    }

    /// [`store`](Self::store) for bytes in a shared buffer: the buffer this world
    /// last read or wrote there is not hashed again, so an unchanged Dust2 navmesh
    /// costs the disk check alone.
    pub fn store_shared(
        &self,
        path: &Path,
        want: Option<&Arc<[u8]>>,
        tag: &str,
    ) -> Result<Saved, String> {
        self.store_want(path, want.map(Want::Shared), tag)
    }

    fn store_want(&self, path: &Path, want: Option<Want>, tag: &str) -> Result<Saved, String> {
        let path = key(path);
        let Some(seen) = self.seen.borrow().get(&path).cloned() else {
            return self.bring(&path, want, None);
        };
        let want_hash = want.map(|w| w.hash(Some(&seen)));
        let differs = want.is_some() && seen.held != want_hash;
        let as_held = seen.mark == Mark::Loaded && seen.held == want_hash;
        let kept = |warning| Saved {
            action: Action::Kept,
            warning,
        };
        match (change(&path, seen.disk), want) {
            (Change::Same, _) if as_held => Ok(kept(None)),
            (Change::Same, _) => self.bring(&path, want, None),
            (Change::Newer, _) if seen.mark == Mark::Baked || differs => {
                let done = if want.is_some() {
                    "overwrote"
                } else {
                    "removed"
                };
                let warning = format!(
                    "[{tag}] {} had changed on disk since this scene read it (another \
                     process baked it?): this scene's own bake {done} it, the last write wins",
                    path.display()
                );
                self.bring(&path, want, Some(warning))
            }
            // A newer file is never removed or written over with what was only loaded,
            // nor because the file it replaced was refused.
            (Change::Newer, _) => Ok(kept(self.warn_newer(&path, tag))),
            (Change::Removed, Some(_)) => {
                let warning = format!(
                    "[{tag}] {} was removed outside the engine: this scene wrote its bake back",
                    path.display()
                );
                self.bring(&path, want, Some(warning))
            }
            (Change::Removed, None) => {
                self.loaded(&path, None, None);
                Ok(kept(None))
            }
        }
    }

    /// Write `want` at `path` (atomically), or remove the file there, and record it.
    fn bring(
        &self,
        path: &Path,
        want: Option<Want>,
        warning: Option<String>,
    ) -> Result<Saved, String> {
        let action = match want {
            Some(w) => {
                // lint: allow(asset-write) `.occlusion` / `.navmesh` bake files, read whole by the scene load (`BakeFiles`), no path-keyed cache
                rusty_core::fs::write_atomic(path, w.bytes())
                    .map_err(|e| format!("Failed to write {}: {e}", path.display()))?;
                Action::Written
            }
            None if path.exists() => {
                std::fs::remove_file(path)
                    .map_err(|e| format!("Failed to remove {}: {e}", path.display()))?;
                rusty_core::fs::sync_folder(path);
                Action::Removed
            }
            None => Action::Kept,
        };
        match want.and_then(|w| w.buffer()) {
            Some(shared) => self.loaded_shared(path, Some(&shared)),
            None => self.loaded(path, want.map(|w| w.bytes()), want.map(|w| w.bytes())),
        }
        if let Some(text) = &warning {
            // lint: allow(log-tag) `text` opens with the caller's tag, `[Occlusion]` and the like
            log::warn!("{text}");
        }
        Ok(Saved { action, warning })
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
    pub fn warned(&self, path: &Path) -> bool {
        self.warned.borrow().contains(&key(path))
    }

    /// Whether this world holds a record for `path`.
    #[cfg(test)]
    pub fn has(&self, path: &Path) -> bool {
        self.seen.borrow().contains_key(&key(path))
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod race_tests;

#[cfg(test)]
mod refused_tests;

#[cfg(test)]
mod replace_tests;

impl crate::Scene {
    /// What this world last read or wrote at each bake sidecar's path: what a save
    /// keeps, read by `dev::bake`'s tests (the field itself is written only here).
    pub fn bake_files(&self) -> &BakeFiles {
        &self.bake_files
    }
}
