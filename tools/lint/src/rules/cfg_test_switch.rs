//! `cfg-test-switch`: live code never switches on `cfg!(test)`.
//!
//! `cfg!(test)` is true only while a crate builds its *own* unit tests. In a
//! workspace, every other crate's tests (and `tests/`, the root's integration
//! binary) link the crate's normal build, where it is false: a behaviour meant
//! for "under test" silently never happens for them. A switch a test elsewhere
//! relies on is a test hook instead, compiled in every build and off until a test
//! arms it (`crates/CLAUDE.md`, *Test hooks*). A `cfg!(…)` naming `test` on a
//! production line of an engine source (`test_code.rs` says which lines are
//! tests) is a finding; one that only the crate's own tests need says why with
//! `// lint: allow(cfg-test-switch) <why>`. A `#[cfg(test)]` gate is not a switch
//! and never counts.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "cfg-test-switch";

/// Every production line of `path` (`test`, per line) holding a `cfg!` that names
/// `test`.
pub fn check_file(path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
    let at = crate::modules::module_path(path);
    if !at.starts_with("src/") {
        return Vec::new();
    }
    code_lines(source)
        .iter()
        .enumerate()
        .filter(|(i, code)| !test.get(*i).copied().unwrap_or(false) && switches(code))
        .map(|(i, _)| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: RULE,
            message: "`cfg!(test)` is false for every other crate's tests; make it a \
                      test hook armed at run time (crates/CLAUDE.md, *Test hooks*), or \
                      allow it inline saying why only this crate's own tests need it"
                .to_string(),
        })
        .collect()
}

/// Does `code` hold a `cfg!(…)` whose predicate names the word `test`?
fn switches(code: &str) -> bool {
    code.match_indices("cfg!(").any(|(at, open)| {
        let rest = &code[at + open.len()..];
        let mut depth = 1;
        let end = rest
            .char_indices()
            .find(|&(_, c)| {
                depth += match c {
                    '(' => 1,
                    ')' => -1,
                    _ => 0,
                };
                depth == 0
            })
            .map_or(rest.len(), |(i, _)| i);
        rest[..end]
            .split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .any(|word| word == "test")
    })
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(|p, s| check_file(p, s, scan.test_lines(p)))
}

#[cfg(test)]
#[path = "cfg_test_switch_tests.rs"]
mod tests;
