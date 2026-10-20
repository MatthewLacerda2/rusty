//! `test-temp-dir`: test code writes under the per-test temp dir (#709, #999).
//!
//! A path under `std::env::temp_dir()` is shared by every test process and every
//! worktree, and under `cargo test` (cargo-mutants) by every test thread of one
//! process, so a fixed or process-keyed name races. In-crate tests use
//! `crate::test_temp::dir()`, integration tests `crate::temp::dir()`: both key on
//! the process and the test. The two helpers themselves, and a Unix socket path
//! (which must stay short), carry an inline allow.
//!
//! Test code is what `test_code.rs` says it is. Comment lines never count, so a doc may name the call.

use super::finding::Finding;
use super::test_code;

pub const RULE: &str = "test-temp-dir";
const CALL: &str = "std::env::temp_dir()";

/// Every `std::env::temp_dir()` call in the test code of `path`.
pub fn check_file(path: &str, source: &str) -> Vec<Finding> {
    let Some(first) = test_code::first_line(path, source) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for (i, line) in source.lines().enumerate().skip(first) {
        let code = line.trim_start();
        if !code.starts_with("//") && line.contains(CALL) {
            found.push(Finding {
                path: path.to_string(),
                line: i + 1,
                rule: RULE,
                message: "a shared temp path races between tests; use \
                          `crate::test_temp::dir()` (in-crate) or `crate::temp::dir()` \
                          (tests/)"
                    .to_string(),
            });
        }
    }
    found
}
