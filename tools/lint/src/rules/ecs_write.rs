//! `ecs-write`: every write to the World's columns goes through the funnel that
//! stamps the change log (#1705).
//!
//! `ecs::changes` reports what changed only because each write calls
//! `World::touch` (or `touch_entity`) on its way in. A raw mutable hecs access
//! written anywhere else in `src/ecs/` compiles, and every consumer then misses
//! that write. A non-test line of `src/ecs/` that borrows a column mutably through
//! a turbofish (`get::<&mut T>`, `query_one::<(&mut A, …)>`, `query_mut`,
//! `query_one_mut`) or changes the store's shape through `inner.` (`spawn`,
//! `despawn`, `insert`, `remove`, `exchange`, `clear`, with their `_one` / `_at`
//! forms) is a finding. Each funnel line says why it may with
//! `// lint: allow(ecs-write) <why>`, and so does `World::derived_mut`, the one
//! write that stamps nothing on purpose. The receiver of a turbofish is not read,
//! so `self\n.inner\n.query_one::<(&mut …)>` across lines still counts; a
//! structural call is matched on `inner.` and must keep it on one line.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "ecs-write";

/// hecs calls whose turbofish borrows a column mutably when it names `&mut`.
const BORROWS: &[&str] = &["get::<", "query::<", "query_one::<"];
/// hecs calls that borrow mutably whatever their type parameter.
const ALWAYS_MUT: &[&str] = &["query_mut", "query_one_mut"];
/// `inner.<call>` that changes which columns an entity has.
const SHAPES: &[&str] = &[
    "spawn",
    "spawn_at",
    "despawn",
    "insert",
    "insert_one",
    "remove",
    "remove_one",
    "exchange",
    "exchange_one",
    "clear",
];

/// Every non-test line of `source` with a raw write to the World's store.
pub fn check(path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
    let at = crate::modules::module_path(path);
    if !at.starts_with("src/ecs/") {
        return Vec::new();
    }
    code_lines(source)
        .iter()
        .enumerate()
        .filter(|(i, code)| !test.get(*i).copied().unwrap_or(false) && raw_write(code))
        .map(|(i, _)| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: RULE,
            message: "a raw column write stamps no change: go through `component_mut`, \
                      `set_component` or `take_component` (`ecs::changes`)"
                .to_string(),
        })
        .collect()
}

/// Does `code` borrow a column mutably or reshape the store, raw?
fn raw_write(code: &str) -> bool {
    let borrow = BORROWS
        .iter()
        .any(|call| word_at(code, call).any(|at| code[at + call.len()..].contains("&mut")));
    let always = ALWAYS_MUT
        .iter()
        .any(|call| word_at(code, call).any(|at| !ident(code[at + call.len()..].chars().next())));
    let shape = SHAPES.iter().any(|call| {
        let full = format!("inner.{call}");
        code.match_indices(&full).any(|(at, _)| {
            let after = code[at + full.len()..].chars().next();
            !ident(after) && !ident(code[..at].chars().next_back())
        })
    });
    borrow || always || shape
}

/// Where `word` starts in `code` with no identifier character before it.
fn word_at<'a>(code: &'a str, word: &'a str) -> impl Iterator<Item = usize> + 'a {
    code.match_indices(word)
        .map(|(at, _)| at)
        .filter(move |&at| !ident(code[..at].chars().next_back()))
}

fn ident(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(|p, s| check(p, s, scan.test_lines(p)))
}

#[cfg(test)]
#[path = "ecs_write_tests.rs"]
mod tests;
