//! `platform-cfg`: rusty ships for macOS and Linux, and nothing else.
//!
//! Windows is not a target (CLAUDE.md, *Ships for macOS and Linux*), so a `cfg`
//! naming it is code nobody builds, and a `unix` one is true on every target rusty
//! has. A code line of a scanned `.rs` file whose `cfg(…)`, `cfg!(…)` or
//! `cfg_attr(…)` names `windows` or `unix` (bare, as `target_os`, or as
//! `target_family`) fails: write the code unconditionally, and split a real
//! macOS / Linux difference with `target_os = "macos"` / `"linux"`. A `cfg` in a
//! comment or a string literal is not code and passes.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::{code_of, is_ident, Open};

pub const RULE: &str = "platform-cfg";

/// The words a predicate may not name.
const UNSHIPPED: &[&str] = &["windows", "unix", "target_family"];

/// Every code line of `source` with a `cfg` that names a platform rusty never
/// ships for, or one family true of every platform it does.
pub fn check(path: &str, source: &str) -> Vec<Finding> {
    let mut open = Open::None;
    let mut found = Vec::new();
    for (i, line) in source.lines().enumerate() {
        let mut code = vec![false; line.len()];
        code_of(line, &mut open, |at, _| code[at] = true);
        if cfgs(line, &code).any(|p| names_unshipped(p)) {
            found.push(Finding {
                path: path.to_string(),
                line: i + 1,
                rule: RULE,
                message: "rusty ships for macOS and Linux only: drop the `windows` / `unix` \
                          `cfg` and write the code unconditionally, or split a real difference \
                          with `target_os = \"macos\"` / `\"linux\"` (CLAUDE.md, *Ships for \
                          macOS and Linux*)"
                    .to_string(),
            });
        }
    }
    found
}

/// The predicate text of each `cfg(`, `cfg!(` or `cfg_attr(` that starts in code,
/// up to its balancing `)` (or the line's end).
fn cfgs<'a>(line: &'a str, code: &'a [bool]) -> impl Iterator<Item = &'a str> + 'a {
    line.match_indices("cfg").filter_map(move |(at, _)| {
        let before = line[..at].chars().next_back();
        if !code.get(at).copied().unwrap_or(false) || before.is_some_and(is_ident) {
            return None;
        }
        let rest = &line[at + 3..];
        let rest = rest
            .strip_prefix('!')
            .or_else(|| rest.strip_prefix("_attr"))
            .unwrap_or(rest);
        let body = rest.strip_prefix('(')?;
        let mut depth = 1;
        let end = body
            .char_indices()
            .find(|&(_, c)| {
                depth += match c {
                    '(' => 1,
                    ')' => -1,
                    _ => 0,
                };
                depth == 0
            })
            .map_or(body.len(), |(end, _)| end);
        Some(&body[..end])
    })
}

/// Does `predicate` name one of [`UNSHIPPED`] as a whole word?
fn names_unshipped(predicate: &str) -> bool {
    let word = |c: Option<char>| c.is_some_and(|c| is_ident(c) || c == '-');
    UNSHIPPED.iter().any(|name| {
        predicate.match_indices(name).any(|(at, _)| {
            !word(predicate[..at].chars().next_back())
                && !word(predicate[at + name.len()..].chars().next())
        })
    })
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(check)
}

#[cfg(test)]
#[path = "platform_cfg_tests.rs"]
mod tests;
