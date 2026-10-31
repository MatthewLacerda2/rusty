//! `subprocess`: the engine never starts another program (#975).
//!
//! A shipped game runs on what it carries. The engine starts Blender, to convert a
//! `.blend` while authoring, and debug builds run `git` for the engine commit
//! (`core::project::engine::commit`), each allowed inline (CLAUDE.md, *One format
//! inside*; `asset::convert::blend`, #1072). A line of `src/` that names
//! `std::process::Command` (`process::` and the word `Command` on one code line, so
//! `use std::process::{Command, Stdio}` counts) is a finding; a legitimate use says
//! why with `// lint: allow(subprocess) <why>`. `tests/` and `tools/` are not the
//! engine and are not scanned.
//!
//! A rule, not clippy's `disallowed-methods`: the platform modules opt out of that
//! lint wholesale for the clock ban (`determinism.rs`), which would exempt them here
//! too, and nothing else may name it.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "subprocess";

/// Every line of an engine source that names `std::process::Command`.
pub fn check(path: &str, source: &str) -> Vec<Finding> {
    if !path.starts_with("src/") {
        return Vec::new();
    }
    code_lines(source)
        .iter()
        .enumerate()
        .filter(|(_, code)| code.contains("process::") && has_word(code.as_ref(), "Command"))
        .map(|(i, _)| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: RULE,
            message:
                "the engine starts no other program but Blender, while authoring (CLAUDE.md, *One format inside*)"
                    .to_string(),
        })
        .collect()
}

/// Does `word` occur in `code` with no identifier character on either side?
fn has_word(code: &str, word: &str) -> bool {
    let ident = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
    code.match_indices(word).any(|(at, _)| {
        !ident(code[..at].chars().next_back()) && !ident(code[at + word.len()..].chars().next())
    })
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(check)
}

#[cfg(test)]
#[path = "subprocess_tests.rs"]
mod tests;
