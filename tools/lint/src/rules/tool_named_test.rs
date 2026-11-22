use super::finding::Finding;
use super::scan::Scan;

pub const RULE: &str = "tool-named-test";
const TOOL_WORDS: &[&str] = &["survivor", "mutant", "mutation"];

pub fn is_tool_named(path: &str, test_only: bool) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    let Some(stem) = name.strip_suffix(".rs") else {
        return false;
    };
    let test_file = test_only
        || stem.ends_with("_tests")
        || path.starts_with("tests/")
        || path.contains("/tests/");
    test_file
        && TOOL_WORDS
            .iter()
            .any(|w| stem.to_lowercase().starts_with(w))
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.files
        .iter()
        .filter(|p| {
            let lines = scan.test_lines(p);
            is_tool_named(p, !lines.is_empty() && lines.iter().all(|&t| t))
        })
        .map(|p| Finding {
            path: p.clone(),
            line: 1,
            rule: RULE,
            message: "a test file is named for the behaviour it tests, not for the tool that \
                      found the gap: rename it (and its `mod` line) for what its tests prove"
                .into(),
        })
        .collect()
}

#[cfg(test)]
#[path = "tool_named_test_tests.rs"]
mod tests;
