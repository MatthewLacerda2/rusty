use super::finding::Finding;
use super::scan::Scan;
use crate::code::code_lines;

pub const RULE: &str = "test-temp-dir";
const CALL: &str = "std::env::temp_dir()";

pub fn check_file(path: &str, source: &str, test: &[bool]) -> Vec<Finding> {
    let mut found = Vec::new();
    for (i, code) in code_lines(source)
        .iter()
        .enumerate()
        .filter(|(i, _)| test[*i])
    {
        if code.contains(CALL) {
            found.push(Finding {
                path: path.to_string(),
                line: i + 1,
                rule: RULE,
                message: "a shared temp path races between tests; use \
                          `rusty_test_support::temp::dir()` (in-crate) or \
                          `crate::temp::dir()` (tests/)"
                    .to_string(),
            });
        }
    }
    found
}

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(|p, s| check_file(p, s, scan.test_lines(p)))
}

#[cfg(test)]
#[path = "temp_dir_tests.rs"]
mod tests;
