//! A scratch directory owned by one test (#629, #709).
//!
//! Under nextest every test is its own process and several run at once (the `gpu`
//! group two at a time), and parallel worktrees share one system temp dir. Under
//! `cargo test` (and cargo-mutants, which runs it) every test of the binary shares
//! one process and runs on its own thread. A fixed path — or one keyed only on the
//! process id — is therefore written by one test while another reads it: a fixture
//! sprite comes back truncated, or an animation graph is half-rewritten when a
//! sibling test loads it. Every test writes under [`dir`] instead, keyed on both the
//! process and the test, so no two tests ever share a file.

use std::path::PathBuf;

/// `<temp>/rusty-tests/<pid>/<test>/`, created on first use. Use it wherever a test
/// would reach for `std::env::temp_dir()`. `<test>` is the running test's name
/// (libtest names each test's thread after it), so a helper shared by several tests
/// can keep a fixed file name and still never collide.
pub fn dir() -> PathBuf {
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
