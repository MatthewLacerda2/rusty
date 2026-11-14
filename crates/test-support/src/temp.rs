//! A scratch directory owned by one test (#629, #709, #999).
//!
//! Under nextest every test is its own process and several run at once, and parallel
//! worktrees share one system temp dir. Under `cargo test` (the nightly `cargo-test.yml`
//! run) every test of the binary shares one process and runs on its own thread.
//! A fixed path under `std::env::temp_dir()` — or one keyed only on the process id —
//! is therefore written by one test while another reads it. In-crate tests write
//! under [`dir`] instead, keyed on both the process and the test. The integration
//! suite re-exports it (`tests/temp.rs`). A pid is reused within one
//! `make test`, so a test's first [`dir`] call clears what a dead process left there
//! (#2078); later calls in the same test keep its files.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, PoisonError};

/// `<temp>/rusty-tests/<pid>/<test>/`, emptied and created on
/// the test's first use, reused by its later calls. `<test>` is the running
/// test's name (libtest names each test's thread after it), so a helper shared by
/// several tests can keep a fixed file name and still never collide.
pub fn dir() -> PathBuf {
    let name = test_name();
    let dir = keyed_path(&name);
    // One lock for the check and the filesystem work: a test name is claimed once.
    let mut claimed = CLAIMED.lock().unwrap_or_else(PoisonError::into_inner);
    if claimed.insert(name) {
        // A pid is reused within one `make test` (thousands of nextest processes), so
        // the folder may hold a dead process's files from the same test (#2078).
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => panic!("clear the test's stale temp dir {}: {e}", dir.display()),
        }
    }
    std::fs::create_dir_all(&dir).expect("create the test's temp dir");
    dir
}

/// `name` inside [`dir`], spelled as the string a script passes: the stored
/// form (`rusty_core::paths::to_slash`, the platform's own spelling, which this
/// crate cannot name from below `core`), so one string serves a Lua literal and the
/// test's own reads and assertions (#1937). Every member's tests and the
/// integration suite use this one.
pub fn lua_path(name: &str) -> String {
    dir().join(name).to_string_lossy().into_owned()
}

/// `dir` spelled relative to the working directory, the way the editor gives
/// `assets`, so a test can hand the engine a relative root (#1332). A fixed `..`
/// count only spells it from one working directory.
pub fn relative(dir: &Path) -> PathBuf {
    let anchor = |p: &Path| -> PathBuf {
        let normal = |c: &Component| matches!(c, Component::Normal(_));
        p.components().take_while(|c| !normal(c)).collect()
    };
    let cwd = std::env::current_dir().expect("the working directory");
    let ups = cwd.strip_prefix(anchor(&cwd)).unwrap().components().count();
    let mut path: PathBuf = std::iter::repeat_n("..", ups).collect();
    path.push(dir.strip_prefix(anchor(dir)).unwrap());
    path
}

/// The test names this process has already handed a folder (and cleared it for).
/// Each concurrent test has its own name, so a first use never clears another's.
static CLAIMED: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());

/// `<temp>/rusty-tests/<pid>/<name>/`, without touching the disk.
fn keyed_path(name: &str) -> PathBuf {
    // lint: allow(test-temp-dir) the per-test helper itself, the one place that keys the path
    std::env::temp_dir()
        .join("rusty-tests")
        .join(std::process::id().to_string())
        .join(name)
}

/// The running test's name as one path component (`a::b` → `a-b`).
fn test_name() -> String {
    let thread = std::thread::current();
    thread.name().unwrap_or("unnamed").replace("::", "-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dead_process_files_are_cleared_on_first_use() {
        let stale = keyed_path(&test_name()).join("late.fbx");
        std::fs::create_dir_all(stale.parent().unwrap()).unwrap();
        std::fs::write(&stale, "left by a dead process with this pid").unwrap();
        let dir = dir();
        assert!(!stale.exists(), "a stale file survived the first dir()");
        assert_eq!(std::fs::read_dir(dir).unwrap().count(), 0);
    }

    #[test]
    fn later_calls_in_one_test_keep_its_files() {
        let file = dir().join("kept.txt");
        std::fs::write(&file, "mine").unwrap();
        assert_eq!(
            std::fs::read_to_string(dir().join("kept.txt")).unwrap(),
            "mine"
        );
    }

    #[test]
    fn a_relative_spelling_names_the_same_folder() {
        let dir = dir();
        let rel = relative(&dir);
        assert!(rel.is_relative(), "{rel:?}");
        let cwd = std::env::current_dir().unwrap();
        let canonical = |p: &Path| p.canonicalize().unwrap();
        assert_eq!(canonical(&cwd.join(&rel)), canonical(&dir), "{rel:?}");
    }
}
