use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "active-self";

pub fn check(path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
    let at = crate::modules::module_path(path);
    if !at.starts_with("src/") || at.starts_with("src/ecs/") {
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

fn reads_self(code: &str) -> bool {
    code.match_indices("is_active_self(").any(|(at, _)| {
        !code[..at]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(|p, s| check(p, s, scan.test_lines(p)))
}

#[cfg(test)]
#[path = "active_self_tests.rs"]
mod tests;
