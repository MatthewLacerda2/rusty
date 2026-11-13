//! `physics-borrow`: a sync pass borrows a rapier body or collider mutably only
//! through the helpers that compare first (#1798).
//!
//! rapier lists every body and collider borrowed mutably as modified, written or
//! not, and that list's order is the order the next step meets new contact
//! pairs, so the order the solver works them. A full sync that borrowed a body
//! the incremental one skipped stepped differently from it.
//! `physics::writes::body_to_write` and `collider_to_write` borrow only when a
//! value differs, so both paths borrow the same handles.
//!
//! A non-test line of `src/physics/` that names `bodies` or `colliders` and
//! borrows from it mutably (`.get_mut(`, `.iter_mut(`, or an index under `&mut`
//! or followed by a `.set_` call or an assignment) is a finding. A `.get_mut(` /
//! `.iter_mut(` that rustfmt wrapped onto its own line counts when the code line
//! above ends in `bodies` or `colliders`. The two helpers, and any line that
//! truly sits outside the sync, say why with `// lint: allow(physics-borrow) <why>`.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "physics-borrow";

/// The rapier sets a sync writes into.
const SETS: &[&str] = &["bodies", "colliders"];
/// Calls that borrow a set's element mutably.
const BORROWS: &[&str] = &[".get_mut(", ".iter_mut("];

/// Every non-test line of `source` that borrows a rapier body or collider raw.
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
        .filter(|&(i, code)| raw_borrow(code, i.checked_sub(1).map(|p| lines[p].as_ref())))
        .map(|(i, _)| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: RULE,
            message: "a raw mutable borrow marks the body or collider modified: borrow \
                      through `body_to_write` / `collider_to_write` (`physics::writes`)"
                .to_string(),
        })
        .collect()
}

/// Does `code` (after the code line `above`) borrow a body or collider mutably?
fn raw_borrow(code: &str, above: Option<&str>) -> bool {
    let wrapped = above.is_some_and(|a| SETS.iter().any(|s| ends_with_word(a.trim_end(), s)))
        && BORROWS.iter().any(|b| code.trim_start().starts_with(b));
    wrapped || SETS.iter().any(|set| set_borrow(code, set))
}

/// Does `code` borrow mutably from `set` on this line?
fn set_borrow(code: &str, set: &str) -> bool {
    word_at(code, set).any(|at| {
        let rest = &code[at + set.len()..];
        if BORROWS.iter().any(|b| rest.starts_with(b)) {
            return true;
        }
        let Some(index) = rest.strip_prefix('[') else {
            return false;
        };
        let after = index
            .find(']')
            .map_or("", |end| index[end + 1..].trim_start());
        let assigned = after.starts_with('=') && !after.starts_with("==");
        code[..at].contains("&mut") || after.starts_with(".set_") || assigned
    })
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
#[path = "physics_borrow_tests.rs"]
mod tests;
