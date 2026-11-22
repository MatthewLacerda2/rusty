//! with `// lint: allow(world-space) <why>`.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "world-space";

const OWNERS: &[&str] = &[
    "src/ecs/world_space/",
    "src/components/transform.rs",
    "src/components/transform/",
];

const WALK: usize = 5;

const TRANSFORM_READS: &[&str] = &[".transform(", "to_matrix(", ".pose()"];

pub fn check(path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
    let at = crate::modules::module_path(path);
    if !at.starts_with("src/") || OWNERS.iter().any(|o| at.starts_with(o)) {
        return Vec::new();
    }
    let code = code_lines(source);
    let code: Vec<&str> = code.iter().map(|c| c.as_ref()).collect();
    let is_test = |i: usize| test.get(i).copied().unwrap_or(false);
    (0..code.len())
        .filter(|&i| !is_test(i))
        .filter_map(|i| why(&code, i).map(|message| (i, message)))
        .map(|(i, message)| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: RULE,
            message: format!("{message}: read it from `ecs::World`'s world API (ecs::world_space)"),
        })
        .collect()
}

fn why(code: &[&str], i: usize) -> Option<&'static str> {
    let line = code[i];
    if line.contains("to_scale_rotation_translation") {
        return Some("decomposes a matrix");
    }
    if has_word(line, "compute_world_matrix") || has_word(line, "parent_world_matrix") {
        return Some("a retired world-matrix walk");
    }
    if line
        .match_indices("world_matrix(")
        .any(|(at, _)| !ident(line[..at].chars().next_back()))
    {
        return Some("a retired world-matrix walk");
    }
    let walk = &code[i..code.len().min(i + WALK + 1)];
    let folds = walk
        .iter()
        .any(|l| TRANSFORM_READS.iter().any(|r| l.contains(r)));
    (line.contains("ancestors(") && folds).then_some("folds a Transform over the hierarchy")
}

fn ident(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

fn has_word(code: &str, word: &str) -> bool {
    code.match_indices(word).any(|(at, _)| {
        !ident(code[..at].chars().next_back()) && !ident(code[at + word.len()..].chars().next())
    })
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(|p, s| check(p, s, scan.test_lines(p)))
}

#[cfg(test)]
#[path = "world_space_tests.rs"]
mod tests;
