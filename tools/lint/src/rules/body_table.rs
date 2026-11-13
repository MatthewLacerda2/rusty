//! `body-table`: a body enters, leaves or changes type in the physics world's
//! table only through the helpers that count the change (#2061).
//!
//! The character controller's sweeps read a cached copy of the body table
//! (`physics::character::bodies`), remade when its change counter moved. A
//! body inserted, removed or retyped without bumping that counter leaves the
//! controller sweeping a stale table: a new floor it walks through, a removed
//! one it still stands on. `PhysicsWorld::insert_body`, `remove_body` and
//! `ControllerBodies::retype` bump it.
//!
//! A non-test line of `src/physics/` that calls `.set_body_type(`, or calls
//! `.insert(` / `.remove(` on `bodies` (on the same line, or on its own line
//! after a code line ending in `bodies`), is a finding. The helpers, and a
//! line that changes a table other than the world's, say why with
//! `// lint: allow(body-table) <why>`.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "body-table";

/// Calls that change which bodies the table holds.
const CHANGES: &[&str] = &[".insert(", ".remove("];

/// Every non-test line of `source` that changes the body table uncounted.
pub fn check(path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
    let at = crate::modules::module_path(path);
    if !at.starts_with("src/physics/") {
        return Vec::new();
    }
    let lines = code_lines(source);
    lines
        .iter()
        .enumerate()
        .filter(|(i, _)| !test.get(*i).copied().unwrap_or(false))
        .filter(|&(i, code)| uncounted(code, i.checked_sub(1).map(|p| lines[p].as_ref())))
        .map(|(i, _)| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: RULE,
            message: "an uncounted body-table change leaves the controller's copy stale: \
                      use `insert_body` / `remove_body` / `ControllerBodies::retype` \
                      (`physics::character::bodies`)"
                .to_string(),
        })
        .collect()
}

/// Does `code` (after the code line `above`) change the body table?
fn uncounted(code: &str, above: Option<&str>) -> bool {
    let wrapped = above.is_some_and(|a| ends_with_word(a.trim_end(), "bodies"))
        && CHANGES.iter().any(|c| code.trim_start().starts_with(c));
    let on_line = word_at(code, "bodies").any(|at| {
        let rest = &code[at + "bodies".len()..];
        CHANGES.iter().any(|c| rest.starts_with(c))
    });
    wrapped || on_line || code.contains(".set_body_type(")
}

/// Where `word` starts in `code` as a whole identifier.
fn word_at<'a>(code: &'a str, word: &'a str) -> impl Iterator<Item = usize> + 'a {
    code.match_indices(word)
        .map(|(at, _)| at)
        .filter(move |&at| {
            !ident(code[..at].chars().next_back()) && !ident(code[at + word.len()..].chars().next())
        })
}

/// Does `code` end in `word` as a whole identifier?
fn ends_with_word(code: &str, word: &str) -> bool {
    code.strip_suffix(word)
        .is_some_and(|head| !ident(head.chars().next_back()))
}

fn ident(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(|p, s| check(p, s, scan.test_lines(p)))
}

#[cfg(test)]
#[path = "body_table_tests.rs"]
mod tests;
