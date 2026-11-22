use super::finding::Finding;
use super::scan::Scan;

pub const RULE: &str = "skip";

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

pub fn run(scan: &Scan) -> Vec<Finding> {
    scan.each_file(check)
}

#[cfg(test)]
#[path = "skip_tests.rs"]
mod tests;
