//! A scratch directory owned by one in-crate test (#629, #709, #999).
//!
//! Under nextest every test is its own process and several run at once, and parallel
//! worktrees share one system temp dir. Under `cargo test` (and cargo-mutants, which
//! runs it) every test of the binary shares one process and runs on its own thread.
//! A fixed path under `std::env::temp_dir()` — or one keyed only on the process id —
//! is therefore written by one test while another reads it. In-crate tests write
//! under [`dir`] instead, keyed on both the process and the test. The integration
//! suite has the same helper (`tests/temp.rs`).

use std::path::PathBuf;

/// `<temp>/rusty-tests/<pid>/<test>/`, created on first use. `<test>` is the running
/// test's name (libtest names each test's thread after it), so a helper shared by
/// several tests can keep a fixed file name and still never collide.
pub(crate) fn dir() -> PathBuf {
    let dir = std::env::temp_dir()
        .join("rusty-tests")
        .join(std::process::id().to_string())
        .join(test_name());
    std::fs::create_dir_all(&dir).expect("create the test's temp dir");
    dir
}

/// The running test's name as one path component (`a::b` → `a-b`).
fn test_name() -> String {
    let thread = std::thread::current();
    thread.name().unwrap_or("unnamed").replace("::", "-")
}
