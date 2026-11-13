//! crates/core/src/fs/written.rs — the log of asset files the engine has (re)written, so
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
//!
//! A change made **outside** the engine (a Blender re-export, an image editor) goes
//! through the same log: the editor's file watch (`shell::editor::watch`, #1327)
//! notes each file it sees change under the project's `assets/` and the engine's
//! shader set, so no cache needs a watch or a per-frame stat of its own.
//!
//! The log holds one entry per distinct path, never forgotten: every file the
//! engine wrote this session and, in the editor, every file seen changing outside
//! it (a rescan after lost events notes every file under the watched folders). Each
//! consumer's [`written_since`] walks the whole log when anything is newer, so its
//! cost grows with the number of distinct files touched in a session, not with how
//! often they change.
//!
//! The rule that keeps it whole: **an engine write of a file some cache reads by
//! path goes through [`write()`] (or, for a writer with its own encoder, calls
//! [`note_written`] right after)**, and **a cache treats a failed load as a miss it
//! retries, never as a cached default**. `rusty_test_support::rebakes::follows_rebakes`
//! is the test every cache runs.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::paths::stored;

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

/// Record that the file at `path` was just written. Returns the note's generation,
/// for a consumer that read the file itself and so can skip this note.
pub fn note_written(path: &str) -> u64 {
    let mut writes = WRITES.lock().unwrap_or_else(|e| e.into_inner());
    writes.0 += 1;
    let generation = writes.0;
    writes.1.insert(path.to_string(), generation);
    drop(writes);
    // A background job may be the writer, and the caches only read this on a frame.
    crate::wake::frame();
    generation
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
/// when it names one of them, by the same [`stored`] form or by another
/// spelling of the same file (relative vs absolute, a symlink). The stored form is
/// what lets a deleted file match (#1937): a note spelled `./assets/a.physmat` and
/// a cache key spelled `assets/a.physmat` name one file, and a file that no longer
/// exists has no canonical form to fall back on. A `\` is part of a name.
pub struct Stale {
    paths: Vec<String>,
    canonical: Vec<PathBuf>,
}

impl Stale {
    pub fn new(paths: Vec<String>) -> Self {
        let canonical = paths.iter().filter_map(|p| canonical(p)).collect();
        let paths = paths.iter().map(|p| stored(p)).collect();
        Self { paths, canonical }
    }

    /// Whether nothing was written: the consumer can skip its sweep.
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Whether cache key `key` names a written file.
    pub fn matches(&self, key: &str) -> bool {
        self.paths.contains(&stored(key))
            || (!self.canonical.is_empty()
                && canonical(key).is_some_and(|c| self.canonical.contains(&c)))
    }
}

/// `path` resolved to an absolute, symlink-free form, if the file exists.
fn canonical(path: &str) -> Option<PathBuf> {
    std::fs::canonicalize(path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_note_wakes_a_sleeping_window() {
        let before = crate::wake::rings();
        note_written("rusty_wake_note");
        assert!(crate::wake::rings() > before);
    }

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
        let dir = rusty_test_support::temp::dir();
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

    #[test]
    fn a_deleted_file_matches_its_key_by_the_stored_form() {
        // The file is gone, so only the stored form can tie the note to the
        // cache's `assets/deleted/gone.physmat` (#1937).
        let stale = Stale::new(vec!["./assets/deleted/gone.physmat".to_string()]);
        assert!(stale.matches("assets/deleted/gone.physmat"));
        assert!(stale.matches("./assets/deleted/gone.physmat"));
        assert!(!stale.matches("assets/deleted/kept.physmat"));
        // A `\` is part of a name, never a separator.
        let stale = Stale::new(vec![r"assets/deleted/a\b.physmat".to_string()]);
        assert!(stale.matches(r"./assets/deleted/a\b.physmat"));
        assert!(!stale.matches("assets/deleted/a/b.physmat"));
    }
}
