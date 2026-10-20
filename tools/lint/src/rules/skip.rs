//! `skip`: a skipped test says why (#935).
//!
//! An ignored test reads as passing to anyone who only reads the summary, so
//! `#[ignore]` and `#[cfg_attr(…, ignore)]` are findings unless an allow gives the
//! reason: `// lint: allow(skip) <why>` on the line or right above it.

use super::finding::Finding;
use super::scan::Scan;

pub const RULE: &str = "skip";

/// Every ignore attribute in `source`.
pub fn check(path: &str, source: &str) -> Vec<Finding> {
    source
        .lines()
        .enumerate()
        .filter(|(_, l)| is_ignore(l.trim_start()))
        .map(|(i, _)| Finding {
            path: path.to_string(),
            line: i + 1,
            rule: RULE,
            message: "a skipped test needs `// lint: allow(skip) <why>`, or remove the skip".into(),
        })
        .collect()
}

fn is_ignore(line: &str) -> bool {
    if line.starts_with("#[ignore") {
        return true;
    }
    line.strip_prefix("#[cfg_attr(").is_some_and(|rest| {
        rest.split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .any(|w| w == "ignore")
    })
}

/// The rule over every scanned file.
pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(check)
}

#[cfg(test)]
#[path = "skip_tests.rs"]
mod tests;
