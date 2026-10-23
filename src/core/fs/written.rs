//! src/core/fs/written.rs — the log of asset files the engine has (re)written, so
//! every runtime cache keyed by path knows which entries went stale (#689, #1207).
//!
//! Unity reimports an asset when it is written; rusty's analog is that every asset
//! file the engine writes (a PNG bake, a `Sound.*` WAV bake, a reflection cubemap,
//! a provider's generated file…) is noted here, and each path-keyed cache (the
//! renderer's textures, cubemaps and particle meshes, the audio `ClipCache`) drains
//! the notes before it answers a lookup. The edit scene follows them too: an entity
//! whose mesh names a re-written model re-reads it (`scene::model_writes`, #1242).
//! A note is data the cache reads, never a call into it, so a sim-side baker never
//! reaches up into `render` or `audio` (the direction rule).
//! The log holds one entry per distinct path, so it stays as small as the set of
//! baked files.
//!
//! The rule that keeps it whole: **an engine write of a file some cache reads by
//! path goes through [`write()`] (or, for a writer with its own encoder, calls
//! [`note_written`] right after)**, and **a cache treats a failed load as a miss it
//! retries, never as a cached default**. `contract::follows_rebakes` is the test
//! every cache runs.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Each written path with the generation of its latest write; the counter is the
/// newest generation handed out. Ordered, so [`written_since`] lists paths by name.
static WRITES: Mutex<(u64, BTreeMap<String, u64>)> = Mutex::new((0, BTreeMap::new()));

/// Write `bytes` at `path` atomically ([`super::write_atomic`]) and note the write.
/// The parent directory must exist. The rename replaces the path itself: a symlink
/// there becomes a regular file, and the file it pointed at is left as it was.
pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    super::write_atomic(path, bytes)?;
    note_written(&path.to_string_lossy());
    Ok(())
}

/// Record that the file at `path` was just written.
pub fn note_written(path: &str) {
    let mut writes = WRITES.lock().unwrap_or_else(|e| e.into_inner());
    writes.0 += 1;
    let generation = writes.0;
    writes.1.insert(path.to_string(), generation);
}

/// The newest write generation, and every path written after generation `seen`. A
/// consumer stores the returned generation and passes it back next time; a new
/// consumer starts from `written_since(u64::MAX).0`, the current generation.
pub fn written_since(seen: u64) -> (u64, Vec<String>) {
    let writes = WRITES.lock().unwrap_or_else(|e| e.into_inner());
    if writes.0 == seen {
        return (seen, Vec::new());
    }
    let fresh = writes
        .1
        .iter()
        .filter(|(_, &g)| g > seen)
        .map(|(p, _)| p.clone());
    (writes.0, fresh.collect())
}

/// The paths a consumer just drained, as a test for its cache keys: a key is stale
/// when it names one of them, by the same string or by another spelling of the same
/// file (relative vs absolute, a symlink).
pub struct Stale {
    paths: Vec<String>,
    canonical: Vec<PathBuf>,
}

impl Stale {
    pub fn new(paths: Vec<String>) -> Self {
        let canonical = paths.iter().filter_map(|p| canonical(p)).collect();
        Self { paths, canonical }
    }

    /// Whether nothing was written: the consumer can skip its sweep.
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Whether cache key `key` names a written file.
    pub fn matches(&self, key: &str) -> bool {
        self.paths.iter().any(|p| p == key)
            || (!self.canonical.is_empty()
                && canonical(key).is_some_and(|c| self.canonical.contains(&c)))
    }
}

/// `path` resolved to an absolute, symlink-free form, if the file exists.
fn canonical(path: &str) -> Option<PathBuf> {
    std::fs::canonicalize(path).ok()
}

#[cfg(test)]
pub(crate) mod contract {
    //! The one test every path-keyed cache runs (#1207).

    /// Which of two distinguishable contents a bake writes.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub(crate) enum Bake {
        A,
        B,
    }

    /// Drive one long-lived cache through the authoring loop at a single `path`:
    /// load before anything is baked (a miss), bake A → load is A, re-bake B → load
    /// is B. `bake` writes through the engine's own writer, `load` goes through the
    /// cache under test, and `is` tells what a load returned. Catches both halves of
    /// the bug kind: a failed load cached for good, and a re-bake that keeps the old
    /// contents.
    pub(crate) fn follows_rebakes<T: std::fmt::Debug>(
        path: &str,
        mut bake: impl FnMut(&str, Bake),
        mut load: impl FnMut(&str) -> T,
        is: impl Fn(&T, Bake) -> bool,
    ) {
        std::fs::remove_file(path).ok();
        let missing = load(path);
        assert!(
            !is(&missing, Bake::A) && !is(&missing, Bake::B),
            "nothing baked yet, got {missing:?}"
        );
        bake(path, Bake::A);
        let first = load(path);
        assert!(is(&first, Bake::A), "a late bake must load, got {first:?}");
        bake(path, Bake::B);
        let second = load(path);
        assert!(is(&second, Bake::B), "a re-bake must load, got {second:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_write_is_reported_once_to_each_consumer() {
        let (start, _) = written_since(0);
        note_written("out/written_test.png");
        let (now, paths) = written_since(start);
        assert!(paths.contains(&"out/written_test.png".to_string()));
        let (_, again) = written_since(now);
        assert!(!again.contains(&"out/written_test.png".to_string()));
        // A second consumer that last looked before the write still sees it.
        assert!(written_since(start)
            .1
            .contains(&"out/written_test.png".to_string()));
    }

    #[test]
    fn a_write_notes_its_path_and_matches_another_spelling_of_it() {
        let dir = crate::test_temp::dir();
        let path = dir.join("noted.bin");
        let (start, _) = written_since(u64::MAX);
        write(&path, b"x").unwrap();
        let (_, paths) = written_since(start);
        let stale = Stale::new(paths);
        assert!(stale.matches(&path.to_string_lossy()));
        let roundabout = dir.join(".").join("noted.bin");
        assert!(stale.matches(&roundabout.to_string_lossy()), "same file");
        assert!(!stale.matches(&dir.join("other.bin").to_string_lossy()));
    }
}
