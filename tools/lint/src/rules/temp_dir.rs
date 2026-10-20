//! `test-temp-dir`: test code writes under the per-test temp dir (#709, #999).
//!
//! A path under `std::env::temp_dir()` is shared by every test process and every
//! worktree, and under `cargo test` (cargo-mutants) by every test thread of one
//! process, so a fixed or process-keyed name races. In-crate tests use
//! `crate::test_temp::dir()`, integration tests `crate::temp::dir()`: both key on
//! the process and the test. The two helpers themselves, and a Unix socket path
//! (which must stay short), carry an inline allow.
//!
//! Test code is everything under `tests/`, a `tests.rs` or `*_tests.rs` file, and a
//! file's lines from its first `#[cfg(test)]` on (the inline `mod tests` shape).
//! Comment lines never count, so a doc may name the call.

use super::finding::Finding;

pub const RULE: &str = "test-temp-dir";
const CALL: &str = "std::env::temp_dir()";

/// Is the whole of `path` test code?
fn test_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    path.starts_with("tests/") || name == "tests.rs" || name.ends_with("_tests.rs")
}

/// Every `std::env::temp_dir()` call in the test code of `path` (under `src/` or
/// `tests/`; other roots are not the engine's tests).
pub fn check_file(path: &str, source: &str) -> Vec<Finding> {
    if !(path.starts_with("src/") || path.starts_with("tests/")) {
        return Vec::new();
    }
    let mut in_test = test_file(path);
    let mut found = Vec::new();
    for (i, line) in source.lines().enumerate() {
        let code = line.trim_start();
        if code.starts_with("#[cfg(test)]") {
            in_test = true;
        }
        if in_test && !code.starts_with("//") && line.contains(CALL) {
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
