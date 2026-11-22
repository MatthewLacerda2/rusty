//! Unity reimports an asset when it is written; here every asset file the engine writes
//! is noted, and each path-keyed cache drains the notes before answering a lookup. An
//! engine write of a file some cache reads by path goes through [`write()`] (or calls
//! [`note_written`] after), and a cache treats a failed load as a retried miss, never a
//! cached default. `rusty_test_support::rebakes::follows_rebakes` tests a cache for this.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::paths::stored;

static WRITES: Mutex<(u64, BTreeMap<String, u64>)> = Mutex::new((0, BTreeMap::new()));

pub fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    super::write_atomic(path, bytes)?;
    note_written(&path.to_string_lossy());
    Ok(())
}

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

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    pub fn matches(&self, key: &str) -> bool {
        self.paths.contains(&stored(key))
            || (!self.canonical.is_empty()
                && canonical(key).is_some_and(|c| self.canonical.contains(&c)))
    }
}

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
