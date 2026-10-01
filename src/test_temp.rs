//! A scratch directory owned by this test process (#629), for in-crate tests.
//!
//! Under nextest every test is its own process and several run at once, and parallel
//! worktrees share one system temp dir, so a fixed path under `std::env::temp_dir()`
//! races: one test rewrites the file while another reads it. In-crate tests write
//! under [`dir`] instead. The integration suite has the same helper (`tests/temp.rs`).

use std::path::PathBuf;

/// `<temp>/rusty-tests/<pid>/`, created on first use.
pub(crate) fn dir() -> PathBuf {
    let dir = std::env::temp_dir()
        .join("rusty-tests")
        .join(std::process::id().to_string());
    std::fs::create_dir_all(&dir).expect("create the test process's temp dir");
    dir
}
