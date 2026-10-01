//! A scratch directory owned by this test process (#629).
//!
//! Under nextest every test is its own process and several run at once (the `gpu`
//! group two at a time), and parallel worktrees share one system temp dir. A fixed
//! path under `std::env::temp_dir()` is therefore written by one test while another
//! reads it: a fixture sprite comes back truncated, the renderer falls back to the
//! checker texture, and a pixel assertion fails. Every test writes under [`dir`]
//! instead, so no two processes ever share a file.

use std::path::PathBuf;

/// `<temp>/rusty-tests/<pid>/`, created on first use. Use it wherever a test would
/// reach for `std::env::temp_dir()`.
pub fn dir() -> PathBuf {
    let dir = std::env::temp_dir()
        .join("rusty-tests")
        .join(std::process::id().to_string());
    std::fs::create_dir_all(&dir).expect("create the test process's temp dir");
    dir
}
