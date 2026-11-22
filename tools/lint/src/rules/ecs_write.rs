//! `// lint: allow(ecs-write) <why>`, and so does `World::derived_mut`, the one

use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "ecs-write";

const BORROWS: &[&str] = &["get::<", "query::<", "query_one::<"];
const ALWAYS_MUT: &[&str] = &["query_mut", "query_one_mut"];
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

fn word_at<'a>(code: &'a str, word: &'a str) -> impl Iterator<Item = usize> + 'a {
    code.match_indices(word)
        .map(|(at, _)| at)
        .filter(move |&at| !ident(code[..at].chars().next_back()))
}

fn ident(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(|p, s| check(p, s, scan.test_lines(p)))
}

#[cfg(test)]
#[path = "ecs_write_tests.rs"]
mod tests;
