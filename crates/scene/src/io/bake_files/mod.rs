//! What this world last read or wrote at each bake sidecar (`<scene>.occlusion`,
//! `<scene>.navmesh`), and the rule a save follows there: `docs/api/Lighting.md`, *Bake
//! files and the scene save*. The SH sidecar is no such file: half the scene document, it
//! is written with it. Keyed by one spelling of each path; interior mutability because a
//! save takes `&Scene`.

mod disk;
mod record;
mod refusals;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use disk::{change, key, Change, Disk, Stamp};
use record::{Mark, Seen, Want};
use rusty_core::hash::content;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Kept,
    Written,
    Removed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    pub action: Action,
    pub warning: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct BakeFiles {
    seen: RefCell<BTreeMap<PathBuf, Seen>>,
    warned: RefCell<BTreeSet<PathBuf>>,
}

impl BakeFiles {
    pub fn loaded(&self, path: &Path, disk: Option<&[u8]>, held: Option<&[u8]>) {
        let stamp = disk.map(Stamp::of);
        let held = match (disk, held, stamp) {
            (Some(d), Some(h), Some(s)) if std::ptr::eq(d, h) => Some(s.hash),
            _ => held.map(content),
        };
        let disk = stamp.map_or(Disk::Missing, Disk::File);
        self.record(path, Seen::loaded(disk, held, None));
    }

    pub fn loaded_shared(&self, path: &Path, bytes: Option<&Arc<[u8]>>) {
        let stamp = bytes.map(|b| Stamp::of(b));
        let disk = stamp.map_or(Disk::Missing, Disk::File);
        let held = stamp.map(|s| s.hash);
        self.record(path, Seen::loaded(disk, held, bytes.cloned()));
    }

    pub fn unreadable(&self, path: &Path, why: &str) {
        let why = Some(format!("could not be read ({why})"));
        let seen = Seen::loaded(Disk::Unreadable, None, None);
        self.record(path, Seen { why, ..seen });
    }

    pub fn baked(&self, suffix: &str) {
        let mut seen = self.seen.borrow_mut();
        let ours = seen
            .iter_mut()
            .filter(|(p, _)| p.as_os_str().to_string_lossy().ends_with(suffix));
        ours.for_each(|(_, s)| {
            s.mark = Mark::Baked;
            s.why = None;
            // Holding the replaced buffer would only keep it alive.
            s.buffer = None;
        });
    }

    fn record(&self, path: &Path, seen: Seen) {
        let path = key(path);
        self.warned.borrow_mut().remove(&path);
        self.seen.borrow_mut().insert(path, seen);
    }

    pub fn store(&self, path: &Path, want: Option<&[u8]>, tag: &str) -> Result<Saved, String> {
        self.store_want(path, want.map(Want::Bytes), tag)
    }

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

    /// A test hook: the dev and edit sides' tests read it.
    pub fn warned(&self, path: &Path) -> bool {
        self.warned.borrow().contains(&key(path))
    }

    /// A test hook: the state inventory's marks read it.
    #[doc(hidden)]
    pub fn has(&self, path: &Path) -> bool {
        self.seen.borrow().contains_key(&key(path))
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod refused_tests;

impl crate::Scene {
    pub fn bake_files(&self) -> &BakeFiles {
        &self.bake_files
    }

    pub fn forget_bake_files(&mut self) {
        self.bake_files = Default::default();
    }

    pub fn restore_bake_files(&mut self, files: BakeFiles) {
        self.bake_files = files;
    }
}
