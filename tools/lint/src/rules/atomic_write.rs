//! `atomic-write`: a file a reader may open is written through the one helper (#1129).
//!
//! `core::fs::write_atomic` writes beside the target under a name unique to the
//! process and the call, renames it into place, and removes its temp file on any
//! failure. Hand-rolled copies drifted (a fixed `.part` name two writers share, a
//! pid-only one two threads share, litter left on failure), so a line of `src/`
//! outside `src/core/fs/` whose code names `fs::rename` is a finding. A rename
//! that is a move, not a write (a staged folder swapped in, a project migrated),
//! says so with `// lint: allow(atomic-write) <why>`. A `rename` imported from
//! `std::fs` (grouped, or aliased with `as`) is the same call, so a bare `rename(`
//! in such a file is a finding too. `tests/` and `tools/` are not the engine and
//! are not scanned.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "atomic-write";
const HOME: &str = "src/core/fs/";

/// The names `std::fs::rename` is imported under: `rename`, or its `as` alias.
///
/// Reads each `use std::fs::...;` statement (it may span lines) and takes the
/// `rename` item of a plain, grouped or aliased import.
fn imported_names(code: &[std::borrow::Cow<str>]) -> Vec<String> {
    let text = code.join("\n");
    let mut names = Vec::new();
    for stmt in text.split(';') {
        let Some(at) = stmt.find("use std::fs::") else {
            continue;
        };
        let tree = &stmt[at + "use std::fs::".len()..];
        for item in tree.split(['{', '}', ',']) {
            let mut words = item.split_whitespace();
            let path = words.next().unwrap_or("");
            if path != "rename" {
                continue;
            }
            match (words.next(), words.next()) {
                (Some("as"), Some(alias)) => names.push(alias.to_string()),
                _ => names.push("rename".to_string()),
            }
        }
    }
    names
}

/// Whether `code` calls `name(` as a free function: not a method, a path tail or a longer word.
fn calls_bare(code: &str, name: &str) -> bool {
    let call = format!("{name}(");
    code.match_indices(&call).any(|(i, _)| {
        let before = code[..i].chars().next_back();
        let ident = |c: char| c.is_alphanumeric() || c == '_';
        let declared = code[..i].trim_end().ends_with("fn");
        !before.is_some_and(|c| ident(c) || c == '.' || c == ':') && !declared
    })
}

/// Every line of an engine source outside [`HOME`] whose code names `fs::rename`,
/// or calls `rename` bare after importing it from `std::fs`.
pub fn check(path: &str, source: &str) -> Vec<Finding> {
    if !path.starts_with("src/") || path.starts_with(HOME) {
        return Vec::new();
    }
    let lines = code_lines(source);
    let names = imported_names(&lines);
    lines
        .iter()
        .enumerate()
        .filter(|(_, code)| {
            code.contains("fs::rename") || names.iter().any(|n| calls_bare(code, n))
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
