//! `atomic-write`: a file a reader may open is written through the one helper (#1129).
//!
//! `core::fs::write_atomic` writes beside the target under a name unique to the
//! process and the call, renames it into place, and removes its temp file on any
//! failure. Hand-rolled copies drifted (a fixed `.part` name two writers share, a
//! pid-only one two threads share, litter left on failure), so a line of `src/`
//! outside `src/core/fs/` whose code names `fs::rename` is a finding. A rename
//! that is a move, not a write (a staged folder swapped in, a project migrated),
//! says so with `// lint: allow(atomic-write) <why>`. `tests/` and `tools/` are not
//! the engine and are not scanned.

use super::finding::Finding;
use super::scan::Scan;

pub const RULE: &str = "atomic-write";
const HOME: &str = "src/core/fs/";

/// Every line of an engine source outside [`HOME`] whose code names `fs::rename`.
pub fn check(path: &str, source: &str) -> Vec<Finding> {
    if !path.starts_with("src/") || path.starts_with(HOME) {
        return Vec::new();
    }
    source
        .lines()
        .enumerate()
        .filter(|(_, l)| {
            let code = l.split("//").next().unwrap_or_default();
            code.contains("fs::rename")
        })
        .map(|(i, _)| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: RULE,
            message: "write through `core::fs::write_atomic`, the one atomic write; \
                      a rename that moves rather than writes takes an inline allow"
                .to_string(),
        })
        .collect()
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(check)
}

#[cfg(test)]
#[path = "atomic_write_tests.rs"]
mod tests;
