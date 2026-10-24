//! `slash-join`: test code never joins a `/`-separated literal onto a path (#1332).
//!
//! `Path::join("a/b")` keeps the `/` as written, so on Windows it builds `dir\a/b`
//! while the engine, joining one part at a time, builds `dir\a\b`. The two compare
//! equal as `Path`s but not as strings, and a test comparing strings (a pending
//! set, a written `.mcp.json`, a log line) fails on Windows only. Four Windows
//! runs turned up such a test one at a time (#1263, #1303, #1306, #1331). Test
//! code joins one part per call, or spells a multi-part path with
//! `crate::test_path::native("a/b")`, the path as the engine writes it.
//!
//! A `.join("…")` whose literal holds a `/` on a test-code line is a finding.
//! Production code is out of scope: it builds what the tests compare against, so
//! a slip there shows up here as a red Windows run. A literal that must keep its
//! `/` (a project-relative asset path is `/`-separated by design) says why with
//! `// lint: allow(slash-join) <why>`.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "slash-join";
const CALL: &str = ".join(\"";

/// Every test-code line of `path` (`test`, per line) joining a literal with a `/`.
pub fn check_file(path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
    code_lines(source)
        .iter()
        .enumerate()
        .filter(|(i, code)| test[*i] && joins_a_slash(code))
        .map(|(i, _)| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: RULE,
            message: "a joined `/` literal builds a mixed-separator path on Windows; \
                      join one part per call, or use `crate::test_path::native(\"a/b\")`"
                .to_string(),
        })
        .collect()
}

/// Does `code` call `.join` on a string literal holding a `/` and more?
fn joins_a_slash(code: &str) -> bool {
    code.match_indices(CALL).any(|(at, _)| {
        let literal = &code[at + CALL.len()..];
        // `parts.join("/")` is a slice joined with a separator, not a path.
        literal
            .split('"')
            .next()
            .is_some_and(|l| l != "/" && l.contains('/'))
    })
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(|p, s| check_file(p, s, scan.test_lines(p)))
}

#[cfg(test)]
#[path = "slash_join_tests.rs"]
mod tests;
