//! `active-self`: systems ask whether an entity is active in the hierarchy (#1363).
//!
//! An inactive parent hides and stops its whole subtree, as Unity's
//! `activeInHierarchy` does. Every system read the entity's own flag instead, so a
//! holstered weapon's children kept drawing under their hidden pivot.
//! `World::is_active_in_hierarchy` is the one answer now: a non-test line of engine
//! code outside `src/ecs/` that calls `is_active_self(` is a finding. What shows or
//! stores the flag itself (the inspector checkbox, `Scene.IsActive`) says so with
//! `// lint: allow(active-self) <why>`.

use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "active-self";

/// Every non-test line of `source` that reads an entity's own active flag.
pub fn check(path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
    if !path.starts_with("src/") || path.starts_with("src/ecs/") {
        return Vec::new();
    }
    code_lines(source)
        .iter()
        .enumerate()
        .filter(|(i, code)| !test.get(*i).copied().unwrap_or(false) && reads_self(code))
        .map(|(i, _)| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: RULE,
            message: "an inactive ancestor hides this entity too: ask `is_active_in_hierarchy`"
                .to_string(),
        })
        .collect()
}

/// Does `code` call `is_active_self(` (not a longer name ending in it)?
fn reads_self(code: &str) -> bool {
    code.match_indices("is_active_self(").any(|(at, _)| {
        !code[..at]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(|p, s| check(p, s, scan.test_lines(p)))
}

#[cfg(test)]
#[path = "active_self_tests.rs"]
mod tests;
