//! `world-space`: nothing but the Transform derives world space (#1170, #1196).
//!
//! Fifteen bugs in two days came from systems working out where an entity is in the
//! world and getting a hierarchy case wrong (a mirror, a zero scale, a turned child
//! under a non-uniform parent). `ecs::World`'s world API (`ecs::world_space`) is the
//! one answer now, so a line of engine code outside it and `components::transform`
//! that re-derives one is a finding:
//! - decomposing a matrix (`to_scale_rotation_translation`), which splits a mirror and
//!   a shear the way #1119 and #1156 got wrong;
//! - the retired names `compute_world_matrix`, `world_matrix(` and `parent_world_matrix`;
//! - folding a Transform over `ancestors(` (a Transform read within the walk's
//!   statement). A walk for membership or depth reads no Transform and passes.
//!
//! Test code is not scanned: a test may decompose a result to check it. Converting
//! file data (an FBX basis, a rig's bind pose) is not the scene hierarchy, and says so
//! with `// lint: allow(world-space) <why>`.

use super::finding::Finding;
use super::scan::Scan;

pub const RULE: &str = "world-space";

/// Where world space is derived: the store and API, and the Transform's own math.
const OWNERS: &[&str] = &[
    "src/ecs/world_space/",
    "src/components/transform.rs",
    "src/components/transform/",
];

/// How many lines after an `ancestors(` still belong to its walk.
const WALK: usize = 5;

/// What reading a Transform looks like inside a walk.
const TRANSFORM_READS: &[&str] = &[".transform(", "to_matrix(", ".pose()"];

/// Every line of `source` that derives world space; `test` marks its test lines.
pub fn check(path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
    if !path.starts_with("src/") || OWNERS.iter().any(|o| path.starts_with(o)) {
        return Vec::new();
    }
    let code: Vec<&str> = source
        .lines()
        .map(|l| l.split("//").next().unwrap_or_default())
        .collect();
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

/// Why line `i` derives world space, or `None`.
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

/// Does `word` occur in `code` with no identifier character on either side?
fn has_word(code: &str, word: &str) -> bool {
    code.match_indices(word).any(|(at, _)| {
        !ident(code[..at].chars().next_back()) && !ident(code[at + word.len()..].chars().next())
    })
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(|p, s| check(p, s, scan.test_lines(p)))
}

#[cfg(test)]
#[path = "world_space_tests.rs"]
mod tests;
