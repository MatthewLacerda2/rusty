//! `read-keys`: only the binaries an author spends from opt in to provider keys
//! (#974), and each of them still does.
//!
//! `dev::providers::read_keys` lets a process read `*_API_KEY` and the `.env`; a
//! process that never calls it holds no key, which is what keeps every test from
//! spending. So a call anywhere but a binary's `main` (`src/main.rs`,
//! `src/bin/*.rs`) is a finding: a library or test calling it would hand keys to
//! every test in its process. And each of [`MUST_CALL`] missing its call is a
//! finding too, since that binary would silently lose every provider verb.
//! `tools/` is not scanned. A deliberate exception takes
//! `// lint: allow(read-keys) <why>`.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "read-keys";

/// The binaries that read provider keys: the editor (with `dev`), `session` and
/// `session-mcp`. `docs/providers.md` names the same three.
pub const MUST_CALL: &[&str] = &[
    "src/main.rs",
    "src/bin/session.rs",
    "src/bin/session_mcp.rs",
];

/// Is `path` a binary's root, where an opt-in may sit?
fn is_binary(path: &str) -> bool {
    path == "src/main.rs"
        || path
            .strip_prefix("src/bin/")
            .is_some_and(|f| !f.contains('/'))
}

/// The lines of `source` that call `read_keys` (its definition is not a call).
fn calls(source: &str) -> Vec<usize> {
    code_lines(source)
        .iter()
        .enumerate()
        .filter(|(_, code)| code.contains("read_keys(") && !code.contains("fn read_keys("))
        .map(|(i, _)| i + 1)
        .collect()
}

/// Every call outside a binary's root.
pub fn check(path: &str, source: &str) -> Vec<Finding> {
    if is_binary(path) || path.starts_with("tools/") {
        return Vec::new();
    }
    calls(source)
        .into_iter()
        .map(|line| Finding {
            path: path.to_string(),
            line,
            rule: RULE,
            message: "only a binary an author spends from calls `read_keys` (docs/providers.md): \
                      anywhere else it hands provider keys to every test in the process"
                .to_string(),
        })
        .collect()
}

/// A finding when `path`, one of [`MUST_CALL`], no longer opts in.
pub fn missing(path: &str, source: &str) -> Option<Finding> {
    calls(source).is_empty().then(|| Finding {
        path: path.to_string(),
        line: 1,
        rule: RULE,
        message: "this binary must call `rusty::dev::providers::read_keys()` first thing, \
                  or every provider verb in it is refused for want of a key"
            .to_string(),
    })
}

/// The rule over every scanned file, and each of [`MUST_CALL`].
pub fn run(scan: &Scan) -> Vec<Finding> {
    let mut found = scan.each_file(check);
    found.extend(MUST_CALL.iter().filter_map(|p| missing(p, scan.source(p))));
    found
}

#[cfg(test)]
#[path = "read_keys_tests.rs"]
mod tests;
