//! `tool-named-test`: a test file is named for the behaviour it tests, never for the
//! tool that found the gap.
//!
//! A file of tests called `survivors.rs`, `survivor_tests.rs` or `mutants.rs` says
//! where its tests came from, not what they hold, so the next reader can't find them
//! beside the behaviour's other tests. The finding sits on the file's first line; the
//! fix is a rename (`survivors.rs` → `growth.rs`). A module whose subject is
//! itself named so (a filter's survivors) takes `// lint: allow(tool-named-test) <why>`
//! on its first line. Only the file name is checked: whether a test's own name and doc
//! comment speak of behaviour rather than of a mutant is a judgement the independent
//! review holds, not a pattern.

use super::finding::Finding;
use super::scan::Scan;

pub const RULE: &str = "tool-named-test";
/// The words a tool-named test file starts with.
const TOOL_WORDS: &[&str] = &["survivor", "mutant", "mutation"];

/// Whether `path` names a test file after the mutation tool. `test_only` is whether
/// every line of it is test code.
pub fn is_tool_named(path: &str, test_only: bool) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    let Some(stem) = name.strip_suffix(".rs") else {
        return false;
    };
    let test_file = test_only
        || stem.ends_with("_tests")
        || path.starts_with("tests/")
        || path.contains("/tests/");
    test_file
        && TOOL_WORDS
            .iter()
            .any(|w| stem.to_lowercase().starts_with(w))
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.files
        .iter()
        .filter(|p| {
            let lines = scan.test_lines(p);
            is_tool_named(p, !lines.is_empty() && lines.iter().all(|&t| t))
        })
        .map(|p| Finding {
            path: p.clone(),
            line: 1,
            rule: RULE,
            message: "a test file is named for the behaviour it tests, not for the tool that \
                      found the gap: rename it (and its `mod` line) for what its tests prove"
                .into(),
        })
        .collect()
}

#[cfg(test)]
#[path = "tool_named_test_tests.rs"]
mod tests;
